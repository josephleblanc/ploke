use super::App;
use crate::app::view::EventSubscriber;
use crate::{app_state::StateCommand, chat_history::MessageKind};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

// Bring AppEvent and SystemEvent into scope from the parent module tree
use super::AppEvent;
use super::system;
use super::utils::display_file_info;

/// Handle AppEvent routing in a lightweight way. This keeps the UI loop lean.
pub(crate) async fn handle_event(app: &mut App, app_event: AppEvent) {
    // Forward to view components that subscribe to events
    app.conversation.on_event(&app_event);
    app.input_view.on_event(&app_event);
    match app_event {
        AppEvent::MessageUpdated(_) | AppEvent::UpdateFailed(_) => {
            app.sync_list_selection().await;
        }
        AppEvent::IndexingProgress(state) => {
            app.indexing_state = Some(state);
        }
        AppEvent::Ui(_ui_event) => {}
        AppEvent::Llm(_event) => {}
        AppEvent::LlmTool(_event) => {}
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
                system::SystemEvent::ModelSwitched(new_model) => {
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
                system::SystemEvent::ReadQuery {
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
                system::SystemEvent::WriteQuery {
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
                system::SystemEvent::BackupDb {
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
                system::SystemEvent::BackupDb {
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
                system::SystemEvent::LoadDb {
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
                system::SystemEvent::LoadDb {
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
                system::SystemEvent::ReIndex { workspace } => {
                    app.send_cmd(StateCommand::IndexWorkspace {
                        workspace,
                        needs_parse: false,
                    });
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
