use std::sync::Arc;

use ploke_io::path_policy::SymlinkPolicy;
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
        match state.system.read().await.crate_focus.clone() {
            Some(path) => path,
            None => match std::env::current_dir() {
                Ok(current_dir) => {
                    let mut pwd = current_dir;
                    pwd.push(&workspace);
                    pwd
                }
                Err(e) => {
                    tracing::error!("Error resolving current dir: {e}");
                    return;
                }
            },
        }
    };

    // Set crate focus to the resolved target directory and update IO roots
    {
        let mut system_guard = state.system.write().await;
        system_guard.crate_focus = Some(target_dir.clone());
    }
    state
        .io_handle
        .update_roots(
            Some(vec![target_dir.clone()]),
            Some(SymlinkPolicy::DenyCrossRoot),
        )
        .await;

    if needs_parse {
        match run_parse(Arc::clone(&state.db), Some(target_dir.clone())) {
            Ok(_) => tracing::info!(
                "Parse of target workspace {} successful",
                &target_dir.display()
            ),
            Err(e) => {
                tracing::info!("Failure parsing directory from IndexWorkspace event: {}", e);
                return;
            }
        }
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
    if let Some(indexer_task) = state_arc
        && let Ok((callback_manager, db_callbacks, unreg_codes_arc, shutdown)) =
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
