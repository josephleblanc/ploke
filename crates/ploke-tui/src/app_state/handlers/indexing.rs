use std::sync::Arc;

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::parser::run_parse;
use crate::{AppEvent, EventBus};

use super::chat::add_msg_immediate;

pub async fn index_workspace(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    workspace: String,
    needs_parse: bool,
) {
    let (control_tx, control_rx) = tokio::sync::mpsc::channel(4);
    let target_dir = {
        let mut write_guard = state.system.write().await;
        tracing::info!("Processing workspace path: {}", workspace);
        
        let crate_focus = if std::path::Path::new(&workspace).is_absolute() {
            // If workspace is already an absolute path, use it directly
            tracing::info!("Using absolute path: {}", workspace);
            std::path::PathBuf::from(&workspace)
        } else {
            // If workspace is relative, append to current directory
            match std::env::current_dir() {
                Ok(current_dir) => {
                    tracing::info!("Resolving relative path '{}' from current dir: {}", 
                                  workspace, current_dir.display());
                    let mut pwd = current_dir;
                    pwd.push(&workspace);
                    tracing::info!("Resolved to: {}", pwd.display());
                    pwd
                }
                Err(e) => {
                    tracing::error!("Error resolving current dir: {e}");
                    return;
                }
            }
        };
        
        // Verify the directory exists
        if !crate_focus.exists() {
            tracing::error!("Target directory does not exist: {}", crate_focus.display());
            add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Error: Directory does not exist: {}", crate_focus.display()),
                crate::chat_history::MessageKind::SysInfo,
            )
            .await;
            return;
        }
        
        if !crate_focus.is_dir() {
            tracing::error!("Target path is not a directory: {}", crate_focus.display());
            add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Error: Path is not a directory: {}", crate_focus.display()),
                crate::chat_history::MessageKind::SysInfo,
            )
            .await;
            return;
        }
        
        tracing::info!("Setting crate_focus to {}", crate_focus.display());
        write_guard.crate_focus = Some(crate_focus.clone());
        crate_focus
    };

    if needs_parse {
        tracing::info!("Starting parse of workspace: {}", target_dir.display());
        match run_parse(Arc::clone(&state.db), Some(target_dir.clone())) {
            Ok(_) => tracing::info!(
                "Parse of target workspace {} successful",
                &target_dir.display()
            ),
            Err(e) => {
                tracing::error!("Failed to parse directory {}: {}", target_dir.display(), e);
                add_msg_immediate(
                    state,
                    event_bus,
                    Uuid::new_v4(),
                    format!("Error parsing workspace: {}", e),
                    crate::chat_history::MessageKind::SysInfo,
                )
                .await;
                return;
            }
        }
    } else {
        tracing::info!("Skipping parse (needs_parse=false) for workspace: {}", target_dir.display());
    }

    add_msg_immediate(
        state,
        event_bus,
        Uuid::new_v4(),
        "Indexing...".to_string(),
        crate::chat_history::MessageKind::SysInfo,
    )
    .await;

    let event_bus_clone = event_bus.clone();
    let progress_tx = Arc::clone(&event_bus.index_tx);
    let progress_rx = event_bus.index_subscriber();

    let state_arc = state.indexer_task.as_ref().map(Arc::clone);
    if let Some(indexer_task) = state_arc {
        if let Ok((callback_manager, db_callbacks, unreg_codes_arc, shutdown)) =
            ploke_db::CallbackManager::new_bounded(Arc::clone(&indexer_task.db), 1000)
        {
            let counter = callback_manager.clone_counter();
            let callback_handler = std::thread::spawn(move || callback_manager.run());
            let res = tokio::spawn(async move {
                let indexing_result = ploke_embed::indexer::IndexerTask::index_workspace(
                    indexer_task,
                    workspace,
                    progress_tx,
                    progress_rx,
                    control_rx,
                    callback_handler,
                    db_callbacks,
                    counter,
                    shutdown,
                )
                .await;
                tracing::info!("Indexer task returned");
                match indexing_result {
                    Ok(_) => {
                        tracing::info!("Indexer finished successfully");
                        // SSoT: Do not emit AppEvent here; run_event_bus will forward IndexingStatus::Completed
                    }
                    Err(e) => {
                        tracing::warn!("Indexer finished with error: {}", e);
                        // SSoT: Do not emit AppEvent here; run_event_bus will forward IndexingStatus::Cancelled/Failed
                    }
                }
            })
            .await;
            match res {
                Ok(_) => {
                    tracing::info!("Sending Indexing Completed");
                }
                Err(e) => {
                    tracing::warn!(
                        "Sending Indexing Failed with error message: {}",
                        e.to_string()
                    );
                }
            }
            tracing::info!("Indexer task returned");
        }
    }
}

pub async fn pause(state: &Arc<AppState>) {
    if let Some(ctrl) = &mut *state.indexing_control.lock().await {
        let _ = ctrl.send(ploke_embed::indexer::IndexerCommand::Pause).await;
    }
}

pub async fn resume(state: &Arc<AppState>) {
    if let Some(ctrl) = &mut *state.indexing_control.lock().await {
        let _ = ctrl
            .send(ploke_embed::indexer::IndexerCommand::Resume)
            .await;
    }
}

pub async fn cancel(state: &Arc<AppState>) {
    if let Some(ctrl) = &mut *state.indexing_control.lock().await {
        let _ = ctrl
            .send(ploke_embed::indexer::IndexerCommand::Cancel)
            .await;
    }
}
