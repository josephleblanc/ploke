use super::App;
use crate::app::view::EventSubscriber;
use crate::app_state::events::SystemEvent;
use crate::llm2::LlmEvent;
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

        AppEvent::Llm2(_llm_event) => {},

        AppEvent::EndpointsResults { model_id, endpoints } => todo!(),

        AppEvent::ModelsEndpointsResults { model_id, providers } => {
            // Defer selection and overlay close until after we release the borrow on model_browser
            let mut select_after: Option<(String, ArcStr)> = None;
            if let Some(mb) = app.model_browser.as_mut() {
                if let Some(item) = mb.items.iter_mut().find(|i| i.id == model_id) {
                    // Map ProviderEntry -> ModelProviderRow
                    let rows = providers
                        .into_iter()
                        .map(|p| {
                            let supports_tools = p.tool_use;
                            ModelProviderRow {
                                name: p.ep_name.clone(),
                                context_length: p.ep_context_length,
                                input_cost: p.ep_pricing_prompt,
                                output_cost: p.ep_pricing_completion,
                                supports_tools,
                            }
                        })
                        .collect::<Vec<_>>();
                    item.providers = rows;
                    item.loading_providers = false;

                    // If user pressed 's' while loading, compute best provider now
                    if item.pending_select {
                        let provider_choice = item
                            .providers
                            .iter()
                            .find(|p| p.supports_tools)
                            .or_else(|| item.providers.first())
                            .map(|p| p.name.clone());

                        if let Some(pid) = provider_choice {
                            // Defer selection until after borrow ends
                            select_after = Some((item.id.clone(), pid));
                        }
                        item.pending_select = false;
                    }
                }
            }
            if let Some((mid, pid)) = select_after {
                app.apply_model_provider_selection(&mid, Some(&pid));
                app.model_browser = None;
            }
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
        AppEvent::Llm(_event) => {}
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
                    app.active_model_indicator = Some((new_model.clone(), Instant::now()));
                    app.active_model_id = new_model;
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
