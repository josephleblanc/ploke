use crossterm::event::{KeyCode, KeyEvent};

use crate::app::overlay::{OverlayAction, OverlayKind};
use crate::app::view::components::model_browser::{ModelBrowserState, preferred_provider_key};

pub fn handle_model_browser_input(mb: &mut ModelBrowserState, key: KeyEvent) -> Vec<OverlayAction> {
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
                if item.expanded
                    && !item.direct_route
                    && item.providers.is_empty()
                    && !item.loading_providers
                {
                    item.loading_providers = true;
                    actions.push(OverlayAction::RequestModelEndpoints {
                        model_id: item.id.clone(),
                    });
                }
            }
        }
        KeyCode::Char('l') => {
            if let Some(item) = mb.items.get_mut(mb.selected) {
                if item.direct_route {
                    item.expanded = true;
                    mb.provider_select_active = false;
                } else if item.expanded {
                    mb.provider_select_active = true;
                    mb.provider_selected = 0;
                } else {
                    item.expanded = true;
                }
                if !item.direct_route && item.providers.is_empty() && !item.loading_providers {
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
                if item.direct_route {
                    actions.push(OverlayAction::SelectModel {
                        model_id: item.id.clone(),
                        provider: None,
                    });
                } else if item.providers.is_empty() {
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
                    actions.push(OverlayAction::SelectModel {
                        model_id: item.id.clone(),
                        provider: preferred_provider_key(&item.providers),
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::app::view::components::model_browser::ModelBrowserItem;
    use crate::llm::ModelId;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn browser_with_item(direct_route: bool) -> ModelBrowserState {
        ModelBrowserState {
            visible: true,
            keyword: "gemini".to_string(),
            items: vec![ModelBrowserItem {
                id: ModelId::from_str("google/gemini-2.5-flash").expect("model id"),
                name: None,
                context_length: None,
                input_cost: Some(0.0),
                output_cost: Some(0.0),
                supports_tools: true,
                providers: Vec::new(),
                direct_route,
                expanded: false,
                loading_providers: false,
                pending_select: false,
            }],
            selected: 0,
            help_visible: false,
            provider_select_active: false,
            provider_selected: 0,
            vscroll: 0,
            viewport_height: 0,
        }
    }

    #[test]
    fn direct_google_selects_without_requesting_endpoints() {
        let mut browser = browser_with_item(true);

        let actions = handle_model_browser_input(&mut browser, key(KeyCode::Char('s')));

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            OverlayAction::SelectModel { model_id, provider } => {
                assert_eq!(model_id.to_string(), "google/gemini-2.5-flash");
                assert!(provider.is_none());
            }
            other => panic!("expected direct model selection, got {other:?}"),
        }
        assert!(!browser.items[0].loading_providers);
        assert!(!browser.items[0].pending_select);
    }

    #[test]
    fn direct_google_expand_does_not_request_endpoints() {
        let mut browser = browser_with_item(true);

        let actions = handle_model_browser_input(&mut browser, key(KeyCode::Enter));

        assert!(actions.is_empty(), "unexpected actions: {actions:?}");
        assert!(browser.items[0].expanded);
        assert!(!browser.items[0].loading_providers);
    }

    #[test]
    fn openrouter_row_still_requests_endpoints_before_selection() {
        let mut browser = browser_with_item(false);

        let actions = handle_model_browser_input(&mut browser, key(KeyCode::Char('s')));

        assert_eq!(actions.len(), 1);
        assert!(matches!(
            &actions[0],
            OverlayAction::RequestModelEndpoints { model_id }
                if model_id.to_string() == "google/gemini-2.5-flash"
        ));
        assert!(browser.items[0].loading_providers);
        assert!(browser.items[0].pending_select);
    }
}
