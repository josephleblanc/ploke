use crossterm::event::{KeyCode, KeyEvent};

use crate::app::overlay::{OverlayAction, OverlayKind};
use crate::app::view::components::model_browser::ModelBrowserState;

pub fn handle_model_browser_input(
    mb: &mut ModelBrowserState,
    key: KeyEvent,
) -> Vec<OverlayAction> {
    let mut actions = Vec::new();

    use KeyCode::*;
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            if mb.provider_select_active {
                mb.provider_select_active = false;
            } else {
                actions.push(OverlayAction::CloseOverlay(OverlayKind::ModelBrowser));
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if mb.provider_select_active {
                if let Some(item) = mb.items.get(mb.selected)
                    && !item.providers.is_empty()
                {
                    if mb.provider_selected > 0 {
                        mb.provider_selected -= 1;
                    } else {
                        mb.provider_selected = item.providers.len().saturating_sub(1);
                    }
                }
            } else if mb.selected > 0 {
                mb.selected -= 1;
            } else {
                mb.selected = mb.items.len().saturating_sub(1);
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if mb.provider_select_active {
                if let Some(item) = mb.items.get(mb.selected) {
                    let len = item.providers.len();
                    if len > 0 {
                        if mb.provider_selected + 1 < len {
                            mb.provider_selected += 1;
                        } else {
                            mb.provider_selected = 0;
                        }
                    }
                }
            } else if mb.items.is_empty() {
                // nothing
            } else if mb.selected + 1 < mb.items.len() {
                mb.selected += 1;
            } else {
                mb.selected = 0;
            }
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            if mb.provider_select_active {
                if let Some(item) = mb.items.get(mb.selected)
                    && !item.providers.is_empty()
                {
                    let idx = mb.provider_selected.min(item.providers.len() - 1);
                    let chosen = item.providers[idx].provider_key.clone();
                    actions.push(OverlayAction::SelectModel {
                        model_id: item.id.clone(),
                        provider: Some(chosen),
                    });
                }
            } else if let Some(item) = mb.items.get_mut(mb.selected) {
                item.expanded = !item.expanded;
                if item.expanded && item.providers.is_empty() && !item.loading_providers {
                    item.loading_providers = true;
                    actions.push(OverlayAction::RequestModelEndpoints {
                        model_id: item.id.clone(),
                    });
                }
            }
        }
        KeyCode::Char('l') => {
            if let Some(item) = mb.items.get_mut(mb.selected) {
                if item.expanded {
                    mb.provider_select_active = true;
                    mb.provider_selected = 0;
                } else {
                    item.expanded = true;
                }
                if item.providers.is_empty() && !item.loading_providers {
                    item.loading_providers = true;
                    actions.push(OverlayAction::RequestModelEndpoints {
                        model_id: item.id.clone(),
                    });
                }
            }
        }
        KeyCode::Char('h') => {
            if let Some(item) = mb.items.get_mut(mb.selected) {
                if item.expanded && !mb.provider_select_active {
                    item.expanded = false;
                }
                mb.provider_select_active = false;
            }
        }
        KeyCode::Char('s') => {
            if let Some(item) = mb.items.get_mut(mb.selected) {
                if item.providers.is_empty() {
                    if !item.loading_providers {
                        item.loading_providers = true;
                        item.pending_select = true;
                        actions.push(OverlayAction::RequestModelEndpoints {
                            model_id: item.id.clone(),
                        });
                    } else {
                        item.pending_select = true;
                    }
                } else {
                    let tool_provider = item
                        .providers
                        .iter()
                        .find(|p| p.supports_tools)
                        .or_else(|| item.providers.first())
                        .map(|p| p.provider_key.clone());
                    actions.push(OverlayAction::SelectModel {
                        model_id: item.id.clone(),
                        provider: tool_provider,
                    });
                }
            }
        }
        KeyCode::Char('?') => {
            mb.help_visible = !mb.help_visible;
        }
        _ => {}
    }
    actions
}
