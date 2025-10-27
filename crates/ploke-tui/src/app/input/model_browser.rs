use crossterm::event::KeyEvent;
use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};

use crate::ModelId;
use crate::app::App;
use crate::llm::manager::events::endpoint;
use crate::llm::router_only::RouterVariants;
use crate::llm::router_only::openrouter::OpenRouter;
use crate::llm::{LlmEvent, ProviderKey};

pub fn handle_model_browser_input(app: &mut App, key: KeyEvent) {
    let mut chosen_model: Option<ModelId> = None;
    let mut chosen_provider: Option<ProviderKey> = None;
    // Intercept keys for model browser overlay when visible

    if let Some(mb) = app.model_browser.as_mut() {
        use KeyCode::*;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if mb.provider_select_active {
                    mb.provider_select_active = false;
                } else {
                    app.model_browser = None;
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
                    if let Some(item) = mb.items.get_mut(mb.selected)
                        && !item.providers.is_empty()
                    {
                        let idx = mb.provider_selected.min(item.providers.len() - 1);
                        let chosen = item.providers[idx].provider_key.clone();
                        chosen_model = Some(item.id.clone());
                        chosen_provider = Some(chosen);
                    }
                } else if let Some(item) = mb.items.get_mut(mb.selected) {
                    item.expanded = !item.expanded;
                    // On expand, if providers not yet loaded, request endpoints
                    if item.expanded && item.providers.is_empty() && !item.loading_providers {
                        item.loading_providers = true;
                        let model_id = item.id.clone();
                        // TODO: Change to a scoped task
                        tokio::spawn(async move {
                            // NOTE: Temporary glue, hardcoding OpenRouter (later want to
                            // make configurable or generic or something)
                            let router = RouterVariants::OpenRouter(OpenRouter);
                            crate::emit_app_event(
                                LlmEvent::Endpoint(endpoint::Event::Request {
                                    model_key: model_id.key,
                                    router,
                                    variant: model_id.variant,
                                })
                                .into(),
                            )
                            .await;
                        });
                    }
                }
            }
            KeyCode::Char('l') => {
                // Enter provider selection mode on current item; trigger load if needed
                if let Some(item) = mb.items.get_mut(mb.selected) {
                    if item.expanded {
                        mb.provider_select_active = true;
                        mb.provider_selected = 0;
                    } else {
                        item.expanded = true;
                    }
                    if item.providers.is_empty() && !item.loading_providers {
                        item.loading_providers = true;
                        let model_id = item.id.clone();
                        tokio::spawn(async move {
                            let router = RouterVariants::OpenRouter(OpenRouter);
                            crate::emit_app_event(
                                LlmEvent::Endpoint(endpoint::Event::Request {
                                    model_key: model_id.key,
                                    router,
                                    variant: model_id.variant,
                                })
                                .into(),
                            )
                            .await;
                        });
                    }
                }
            }
            KeyCode::Char('h') => {
                // Enter provider selection mode on current item; trigger load if needed
                if let Some(item) = mb.items.get_mut(mb.selected) {
                    if item.expanded && !mb.provider_select_active {
                        item.expanded = false;
                    }
                    mb.provider_select_active = false;
                }
            }
            KeyCode::Char('s') => {
                if let Some(item) = mb.items.get_mut(mb.selected) {
                    chosen_model = Some(item.id.clone());
                    if item.providers.is_empty() {
                        // Fetch endpoints first, then auto-select when results arrive
                        if !item.loading_providers {
                            item.loading_providers = true;
                            item.pending_select = true;
                            let model_id = item.id.clone();
                            // TODO: Change to a scoped task
                            tokio::spawn(async move {
                                let router = RouterVariants::OpenRouter(OpenRouter);
                                crate::emit_app_event(
                                    LlmEvent::Endpoint(endpoint::Event::Request {
                                        model_key: model_id.key,
                                        router,
                                        variant: model_id.variant,
                                    })
                                    .into(),
                                )
                                .await;
                            });
                        } else {
                            // Already loading; just mark pending select
                            item.pending_select = true;
                        }
                    } else {
                        // Choose a provider that supports tools if available, otherwise first provider
                        let tool_provider = item
                            .providers
                            .iter()
                            .find(|p| p.supports_tools)
                            .or_else(|| item.providers.first())
                            .map(|p| p.provider_key.clone());
                        chosen_provider = tool_provider;
                    }
                }
            }
            KeyCode::Char('?') => {
                mb.help_visible = !mb.help_visible;
            }
            _ => {}
        }
    }
    // Drop the mutable borrow of app.model_browser before switching model
    if let Some(model_id) = chosen_model {
        // id format: "model_id::provider_id" when provider selected, or just model_id
        if let Some(provider) = chosen_provider {
            app.apply_model_provider_selection(model_id.to_string(), Some(provider));
        } else {
            app.apply_model_provider_selection(model_id.to_string(), None);
        }
        app.model_browser = None;
    }
    app.needs_redraw = true;
}
