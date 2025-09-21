use super::App;
use crate::app::view::EventSubscriber;
use crate::app_state::events::SystemEvent;
use crate::llm::manager::events::{endpoint, models};
use crate::llm::{LlmEvent, ProviderKey};
use crate::{app_state::StateCommand, chat_history::MessageKind};
use std::sync::Arc;
use std::time::Instant;
use ploke_core::ArcStr;
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

        AppEvent::EndpointsResults { model_id, endpoints } => todo!(),

        AppEvent::ModelsEndpointsResults { model_id, providers } => {
            // Defer selection and overlay close until after we release the borrow on model_browser
            // let mut select_after: Option<(String, ArcStr)> = None;
            // if let Some(mb) = app.model_browser.as_mut() {
            //     if let Some(item) = mb.items.iter_mut().find(|i| i.id == model_id) {
            //         // Map ProviderEntry -> ModelProviderRow
            //         let rows = providers
            //             .into_iter()
            //             .map(|p| {
            //                 let supports_tools = p.tool_use;
            //                 ModelProviderRow {
            //                     name: p.ep_name.clone(),
            //                     context_length: p.ep_context_length,
            //                     input_cost: p.ep_pricing_prompt,
            //                     output_cost: p.ep_pricing_completion,
            //                     supports_tools,
            //                 }
            //             })
            //             .collect::<Vec<_>>();
            //         item.providers = rows;
            //         item.loading_providers = false;
            //
            //         // If user pressed 's' while loading, compute best provider now
            //         if item.pending_select {
            //             let provider_choice = item
            //                 .providers
            //                 .iter()
            //                 .find(|p| p.supports_tools)
            //                 .or_else(|| item.providers.first())
            //                 .map(|p| p.name.clone());
            //
            //             if let Some(pid) = provider_choice {
            //                 // Defer selection until after borrow ends
            //                 select_after = Some((item.id.clone(), pid));
            //             }
            //             item.pending_select = false;
            //         }
            //     }
            // }
            // if let Some((mid, pid)) = select_after {
            //     app.apply_model_provider_selection(&mid, Some(&pid));
            //     app.model_browser = None;
            // }
        }
        AppEvent::MessageUpdated(_) | AppEvent::UpdateFailed(_) => {
            app.sync_list_selection().await;
        }
        AppEvent::ModelSearchResults { keyword, items } => {
            // Populate or update the Model Browser overlay with async results
            app.open_model_browser(keyword, items);
        }
        AppEvent::ModelsEndpointsRequest { .. } => {
            // Request event: handled by llm_manager; UI waits for ModelsEndpointsResults.
        }
        AppEvent::IndexingProgress(state) => {
            app.indexing_state = Some(state);
        }
        AppEvent::Ui(_ui_event) => {}
        AppEvent::Llm(event) => {
            match event {
                LlmEvent::Models(resp @ models::Event::Response { .. }) => {
                    handle_llm_models_response(app, resp)
                }
                LlmEvent::Endpoint(ep_evt) => handle_llm_endpoints_response(app, ep_evt),
                LlmEvent::ChatCompletion(_) => {}
                LlmEvent::Completion(_) => {}
                LlmEvent::Tool(_) => {}
                LlmEvent::Status(_) => {}
                _ => {}
            }
        }
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
            tracing::info!("Indexing Succeeded!");
            app.indexing_state = None;
            app.send_cmd(StateCommand::AddMessageImmediate {
                msg: String::from("Indexing Succeeded"),
                kind: MessageKind::SysInfo,
                new_msg_id: Uuid::new_v4(),
            });
            app.send_cmd(StateCommand::UpdateDatabase)
        }
        AppEvent::IndexingFailed => {
            tracing::error!("Indexing Failed");
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
                    tracing::debug!("SystemEvent::ModelSwitched {}", new_model);
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: format!(
                            "model changed from {} to {}",
                            app.active_model_id, new_model
                        ),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                    app.active_model_indicator = Some((new_model.clone().to_string(), Instant::now()));
                    app.active_model_id = new_model.to_string();
                }
                SystemEvent::ReadQuery {
                    file_name,
                    query_name,
                } => {
                    tracing::debug!("App receives event: {}", file_name);
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
                    tracing::debug!(
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
                    tracing::debug!("App receives HistorySaved: {}", file_path);
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
                    tracing::debug!(
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
                    tracing::debug!(
                        "App receives BackupDb unsuccessful event: {}\nwith error: {:?}",
                        &file_dir,
                        &error
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
                    is_success,
                    ..
                } if is_success => {
                    tracing::debug!(
                        "App receives LoadDb successful db save to file: {:?}",
                        display_file_info(file_dir.as_ref()),
                    );
                    app.send_cmd(StateCommand::AddMessageImmediate {
                        msg: format!("Success: Cozo data for code graph loaded successfully for {crate_name} from {}", 
                            display_file_info(file_dir.as_ref()), 
                        ),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    });
                }
                SystemEvent::LoadDb {
                    crate_name,
                    file_dir,
                    is_success,
                    error,
                } if !is_success => {
                    tracing::debug!(
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
                    tracing::debug!("Test harness API response event received");
                }
                other => {
                    tracing::warn!("Unused system event in main app loop: {:?}", other)
                }
            }
        }
        AppEvent::GenerateContext(_id) => {
            // future hook
        }
    }
}

fn handle_llm_models_response(app: &mut App, models_event: models::Event) {
        
    if let models::Event::Response { models } = models_event {
    }
}

fn handle_llm_endpoints_response(app: &mut App, endpoints_event: endpoint::Event) {
    if let endpoint::Event::Response { model_key: _, endpoints } = endpoints_event {
        let eps = match endpoints {
            Some(epoints) => epoints,
            None => {
                send_warning(app, "No endpoints found for model.");
                return
            },
        };
        let mb = match app.model_browser.as_mut() {
            Some(m_browser) => m_browser,
            None => { 
                send_warning(app, "Querying model endpoints outside of model browser"); 
                return 
            }
        };

        let mut select_after: Option<(crate::llm::ModelId, crate::llm::ProviderKey)> = None;

        let browser_item = match mb.items.iter_mut().find(|i| i.id == eps.data.id) {
            Some(b) => b,
            None => {
                send_error(app, "No matching item for returned model endopints");
                return
            }

        };
        
        let model_id = &eps.data.id;
        // Map ProviderEntry -> ModelProviderRow
        let rows = eps
            .data
            .endpoints
            .iter()
            .map(|ep| {
                let key = ProviderKey { slug: ep.tag.provider_name.clone() };
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
    tracing::warn!(msg);
    app.send_cmd(StateCommand::AddMessageImmediate { msg, kind: MessageKind::SysInfo, new_msg_id: Uuid::new_v4() });
}

fn send_error(app: &mut App, message: &str) {
    let msg = String::from(message);
    tracing::warn!(msg);
    app.send_cmd(StateCommand::AddMessageImmediate { msg, kind: MessageKind::SysInfo, new_msg_id: Uuid::new_v4() });
}
