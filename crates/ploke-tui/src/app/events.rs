use super::App;
use crate::SearchEvent;
use crate::app::view::EventSubscriber;
use crate::app_state::events::SystemEvent;
use ploke_llm::manager::events::{endpoint, models};
use crate::llm::{LlmEvent, ProviderKey};
use crate::{app_state::StateCommand, chat_history::MessageKind};
use itertools::Itertools;
use ploke_core::rag_types::AssembledContext;
use std::time::Instant;
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

// Bring AppEvent and SystemEvent into scope from the parent module tree
use super::AppEvent;
use super::utils::display_file_info;
use crate::app::view::components::model_browser::ModelProviderRow;

/// Handle AppEvent routing in a lightweight way. This keeps the UI loop lean.
pub(crate) async fn handle_event(app: &mut App, app_event: AppEvent) {
    // Forward to view components that subscribe to events
    app.conversation.on_event(&app_event);
    app.input_view.on_event(&app_event);
    match app_event {
        AppEvent::Quit => {
            app.quit();
        }

        // LLM events routed into llm match arm below
        AppEvent::MessageUpdated(_) | AppEvent::UpdateFailed(_) => {
            app.sync_list_selection().await;
        }
        AppEvent::ContextSearch(SearchEvent::SearchResults {
            query_id,
            context: assembled_context,
        }) => {
            tracing::debug!(
                "receieved ContextSearch with assembled_context stats: {:#?}",
                assembled_context.stats
            );
            if let Some(ctx_browser) = app.context_browser.as_mut() {
                if query_id != ctx_browser.query_id {
                    trace!(
                        "Ignoring stale context search results: incoming={} current={}",
                        query_id, ctx_browser.query_id
                    );
                    return;
                }
                let AssembledContext { parts, stats } = assembled_context;
                info!(
                    "ContextSearch event completed with search results.
                    AssembledContext with stats:
                    {stats:#?}"
                );
                ctx_browser.set_results(App::build_context_search_items(parts));
            }
        }
        AppEvent::IndexingProgress(state) => {
            app.indexing_state = Some(state);
        }
        AppEvent::Ui(_ui_event) => {}
        AppEvent::Llm(event) => match event {
            LlmEvent::Models(resp @ models::Event::Response { .. }) => {
                handle_llm_models_response(app, resp)
            }
            LlmEvent::Endpoint(ep_evt) => handle_llm_endpoints_response(app, ep_evt),
            LlmEvent::ChatCompletion(_) => {}
            LlmEvent::Tool(_) => {}
            LlmEvent::Status(_) => {}
            _ => {}
        },
        AppEvent::LlmTool(_event) => {}
        AppEvent::EventBusStarted => {}
        AppEvent::Rag(_rag_event) => {}
        AppEvent::Error(error_event) => {
            let msg = format!("Error: {}", error_event.message);
            app.send_cmd(StateCommand::AddMessageImmediate {
                msg,
                kind: MessageKind::SysInfo,
                new_msg_id: Uuid::new_v4(),
            });
        }
        AppEvent::IndexingStarted => {}
        AppEvent::IndexingCompleted => {
            info!("Indexing Succeeded!");
            app.indexing_state = None;
            app.send_cmd(StateCommand::AddMessageImmediate {
                msg: String::from("Indexing Succeeded"),
                kind: MessageKind::SysInfo,
                new_msg_id: Uuid::new_v4(),
            });
            app.send_cmd(StateCommand::UpdateDatabase)
        }
        AppEvent::IndexingFailed => {
            error!("Indexing Failed");
            app.indexing_state = None;
            app.send_cmd(StateCommand::AddMessageImmediate {
                msg: String::from("Indexing Failed"),
                kind: MessageKind::SysInfo,
                new_msg_id: Uuid::new_v4(),
            })
        }
        // NOTE: This system event handling is a bad pattern. This should probably be
        // managed by the event_bus system instead.
        AppEvent::System(system_event) => {
            match system_event {
                SystemEvent::ModelSwitched(new_model) => {
                    debug!("SystemEvent::ModelSwitched {}", new_model);
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: format!(
                            "model changed from {} to {}",
                            app.active_model_id, new_model
                        ),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                    app.active_model_indicator =
                        Some((new_model.clone().to_string(), Instant::now()));
                    app.active_model_id = new_model.to_string();
                }
                SystemEvent::ReadQuery {
                    file_name,
                    query_name,
                } => {
                    debug!("App receives event: {}", file_name);
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: format!("Reading file for query called {query_name}:\n\t{file_name}"),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                }
                SystemEvent::WriteQuery {
                    query_name,
                    query_content,
                } => {
                    debug!(
                        "App receives WriteQuery from FileManager for {query_name}:\n{query_content}"
                    );
                    let shortened_query = query_content.chars().take(20).collect::<String>();
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: format!("Query read from file with query name {query_name}:\n\t{shortened_query}..."),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                    app.send_cmd(StateCommand::WriteQuery {
                        query_name,
                        query_content,
                    });
                }
                SystemEvent::HistorySaved { file_path } => {
                    debug!("App receives HistorySaved: {}", file_path);
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: format!("Chat history exported to {}", file_path),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                }
                SystemEvent::BackupDb {
                    file_dir,
                    is_success,
                    ..
                } if is_success => {
                    debug!(
                        "App receives BackupDb successful db save to file: {}",
                        &file_dir
                    );
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: format!(
                            "Success: Cozo data for code graph saved successfully to {file_dir}"
                        ),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                }
                SystemEvent::BackupDb {
                    file_dir,
                    is_success,
                    error,
                } if !is_success => {
                    debug!(
                        "App receives BackupDb unsuccessful event: {}\nwith error: {:?}",
                        &file_dir, &error
                    );
                    if let Some(error_str) = error {
                        app.send_cmd(StateCommand::AddMessageImmediate {
                            msg: format!("Error: Cozo data for code graph not saved to {file_dir}\n\tFailed with error: {}", &error_str),
                            kind: MessageKind::SysInfo,
                            new_msg_id: Uuid::new_v4(),
                        });
                    }
                }
                SystemEvent::LoadDb {
                    crate_name,
                    file_dir,
                    root_path,
                    is_success,
                    ..
                } if is_success => {
                    debug!(
                        "App receives LoadDb successful db save to file: {:?}",
                        display_file_info(file_dir.as_ref()),
                    );
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: format!("Success: Cozo data for code graph loaded successfully for {crate_name} from {}\nRoot project path set to: {}", 
                            display_file_info(file_dir.as_ref()),
                            display_file_info(root_path.as_ref())
                        ),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                }
                SystemEvent::LoadDb {
                    crate_name,
                    file_dir,
                    root_path,
                    is_success,
                    error,
                } if !is_success => {
                    debug!(
                        "App receives LoadDb unsuccessful event: {}\nwith error: {:?}",
                        display_file_info(file_dir.as_ref()),
                        &error
                    );
                    if let Some(error_str) = error {
                        app.send_cmd(StateCommand::AddMessageImmediate {
                            msg: format!("Error: Cozo data for code graph of {crate_name} not loaded from {}\n\tFailed with error: {}", 
                                display_file_info(file_dir.as_ref()),
                                &error_str),
                            kind: MessageKind::SysInfo,
                            new_msg_id: Uuid::new_v4(),
                        });
                    }
                }
                SystemEvent::ReIndex { workspace } => {
                    app.send_cmd(StateCommand::IndexWorkspace {
                        workspace,
                        needs_parse: false,
                    });
                }
                #[cfg(all(feature = "test_harness", feature = "live_api_tests"))]
                SystemEvent::TestHarnessApiResponse { .. } => {
                    // Test harness API response - handled by test subscribers, no UI action needed
                    debug!("Test harness API response event received");
                }
                other => {
                    warn!("Unused system event in main app loop: {:?}", other)
                }
            }
        }
        AppEvent::GenerateContext(_id) => {
            // future hook
        }
    }
}

// TODO: Now we are getting another issue, where the model browser is loading the list of returned
// models for the search, filtered correctly, BUT when an item is expanded by pressing `l` on the
// selected item, while there is a filled in "context_length", "supports_tools", "pricing" with
// both "in" and "out", the "providers" field, which should be a list of the various providers for
// that specific model (along with other info like the prices for that provider or other
// variations), we only see a "(loading...)", which never resolves.
fn handle_llm_models_response(app: &mut App, models_event: models::Event) {
    let models::Event::Response {
        models,
        search_keyword,
    } = models_event
    else {
        debug!("Unexpected event type");
        return;
    };

    let Some(models_payload) = models else {
        send_warning(app, "Model search response missing payload.");
        return;
    };

    let Some(mb) = app.model_browser.as_ref() else {
        debug!("Received model list response without open browser");
        return;
    };

    let Some(keyword_snapshot) = app.model_browser.as_ref().map(|mb| mb.keyword.clone()) else {
        debug!("Received model list response without an open browser; ignoring");
        return;
    };

    if let Some(kw) = search_keyword.as_deref().filter(|k| *k != mb.keyword) {
        debug!(
            "Dropping stale results, expected '{}', got '{}')",
            mb.keyword, kw
        );
        return;
    }

    let filtered = filter_models_for_keyword(models_payload.as_ref(), &mb.keyword);
    if filtered.is_empty() {
        let warning = format!(
            "No models matched '{}'. Try a broader search.",
            keyword_snapshot
        );
        send_warning(app, &warning);
    }

    if let Some(mb) = app.model_browser.as_mut() {
        // Drop results if another search replaced the keyword while this async request completed.
        if mb.keyword != keyword_snapshot {
            debug!(
                "Model browser keyword changed from '{}' → '{}' while request was pending",
                keyword_snapshot, mb.keyword,
            );
            return;
        }

        let mapped = App::build_model_browser_items(filtered);
        let previous_selection = mb.selected;
        mb.items = mapped;
        if mb.items.is_empty() {
            mb.selected = 0;
        } else if previous_selection >= mb.items.len() {
            mb.selected = mb.items.len() - 1;
        }
        mb.vscroll = 0;
        app.needs_redraw = true;
    }
}

fn filter_models_for_keyword(
    models: &crate::llm::request::models::Response,
    keyword: &str,
) -> Vec<crate::llm::request::models::ResponseItem> {
    let kw_lower = keyword.to_lowercase();
    let filtered: Vec<_> = models
        .data
        .iter()
        .filter(|model| {
            let id_match = model.id.to_string().to_lowercase().contains(&kw_lower);
            let name_match = model.name.as_str().to_lowercase().contains(&kw_lower);
            id_match || name_match
        })
        .sorted_by_key(|x| &x.id)
        .cloned()
        .collect();
    filtered
}

fn handle_llm_endpoints_response(app: &mut App, endpoints_event: endpoint::Event) {
    if let endpoint::Event::Response {
        model_key: _,
        endpoints,
    } = endpoints_event
    {
        let eps = match endpoints {
            Some(epoints) => epoints,
            None => {
                send_warning(app, "No endpoints found for model.");
                return;
            }
        };
        let mb = match app.model_browser.as_mut() {
            Some(m_browser) => m_browser,
            None => {
                send_warning(app, "Querying model endpoints outside of model browser");
                return;
            }
        };

        let mut select_after: Option<(crate::llm::ModelId, crate::llm::ProviderKey)> = None;

        let browser_item = match mb.items.iter_mut().find(|i| i.id == eps.data.id) {
            Some(b) => b,
            None => {
                send_error(app, "No matching item for returned model endopints");
                return;
            }
        };

        let model_id = &eps.data.id;
        // Map ProviderEntry -> ModelProviderRow
        let rows = eps
            .data
            .endpoints
            .iter()
            .map(|ep| {
                let key = ProviderKey {
                    slug: ep.tag.provider_name.clone(),
                };
                ModelProviderRow::from_id_endpoint(model_id.clone(), &key, ep.clone())
            })
            .collect::<Vec<_>>();
        browser_item.providers = rows;
        browser_item.loading_providers = false;

        // If user pressed 's' while loading, choose a provider automatically now
        if browser_item.pending_select {
            let tool_provider = browser_item
                .providers
                .iter()
                .find(|p| p.supports_tools)
                .or_else(|| browser_item.providers.first())
                .map(|p| p.provider_key.clone());

            if let Some(pk) = tool_provider {
                // Defer actual selection to after we release the borrow on model_browser
                select_after = Some((model_id.clone(), pk));
            }
            browser_item.pending_select = false;
        }

        // explicitly release mutable borrow of model_browser by ending its scope

        if let Some((mid, pk)) = select_after {
            app.apply_model_provider_selection(mid.to_string(), Some(pk));
            app.close_model_browser();
        }
    }
}

fn send_warning(app: &mut App, message: &str) {
    let msg = String::from(message);
    warn!(msg);
    app.send_cmd(StateCommand::AddMessageImmediate {
        msg,
        kind: MessageKind::SysInfo,
        new_msg_id: Uuid::new_v4(),
    });
}

fn send_error(app: &mut App, message: &str) {
    let msg = String::from(message);
    warn!(msg);
    app.send_cmd(StateCommand::AddMessageImmediate {
        msg,
        kind: MessageKind::SysInfo,
        new_msg_id: Uuid::new_v4(),
    });
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn filters_models_by_keyword_case_insensitively() {
        let resp = sample_response();
        let filtered = filter_models_for_keyword(&resp, "gPt");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id.to_string(), "openai/gpt-4o");
    }

    #[test]
    fn returns_all_models_when_keyword_empty() {
        let resp = sample_response();
        let filtered = filter_models_for_keyword(&resp, "");
        assert_eq!(filtered.len(), 2);
    }

    fn sample_response() -> crate::llm::request::models::Response {
        serde_json::from_value(json!({
            "data": [
                {
                    "id": "openai/gpt-4o",
                    "name": "GPT-4o",
                    "created": 0,
                    "description": "primary test model",
                    "architecture": {
                        "input_modalities": ["text"],
                        "modality": "text->text",
                        "output_modalities": ["text"],
                        "tokenizer": "Llama4",
                        "instruct_type": null
                    },
                    "top_provider": {
                        "context_length": 8192,
                        "is_moderated": false,
                        "max_completion_tokens": null
                    },
                    "pricing": {
                        "prompt": "0.000002",
                        "completion": "0.000004"
                    },
                    "canonical_slug": null,
                    "context_length": 8192,
                    "hugging_face_id": null,
                    "per_request_limits": null,
                    "supported_parameters": ["tools"]
                },
                {
                    "id": "meta/llama-3.1",
                    "name": "Meta Llama",
                    "created": 0,
                    "description": "secondary model entry",
                    "architecture": {
                        "input_modalities": ["text"],
                        "modality": "text->text",
                        "output_modalities": ["text"],
                        "tokenizer": "Llama3",
                        "instruct_type": null
                    },
                    "top_provider": {
                        "context_length": 4096,
                        "is_moderated": false,
                        "max_completion_tokens": null
                    },
                    "pricing": {
                        "prompt": "0.000001",
                        "completion": "0.000002"
                    },
                    "canonical_slug": null,
                    "context_length": 4096,
                    "hugging_face_id": null,
                    "per_request_limits": null,
                    "supported_parameters": ["tools"]
                }
            ]
        }))
        .expect("valid response fixture")
    }
}
