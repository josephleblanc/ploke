use std::sync::Arc;
#[cfg(feature = "test_harness")]
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::app_state::{AppState, handlers};
use crate::chat_history::MessageKind;
use crate::error::ErrorSeverity;
use crate::event_bus::ErrorEvent;
use crate::parser::{resolve_index_target, run_parse_resolved};
use crate::utils::parse_errors::format_parse_failure;
use crate::{AppEvent, EventBus};

use super::chat::add_msg_immediate;

#[cfg(feature = "test_harness")]
static INDEXING_TEST_DELAY_MS: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "test_harness")]
pub fn set_indexing_test_delay_ms(delay_ms: u64) {
    INDEXING_TEST_DELAY_MS.store(delay_ms, Ordering::SeqCst);
}

async fn maybe_delay_indexing_for_test() {
    #[cfg(feature = "test_harness")]
    {
        let delay_ms = INDEXING_TEST_DELAY_MS.load(Ordering::SeqCst);
        if delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
    }
}

pub async fn index_workspace(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    workspace: String,
    needs_parse: bool,
) {
    let add_msg_shortcut = |msg: &str| {
        handlers::chat::add_msg_immediate(
            &state,
            &event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
        )
    };
    let (control_tx, control_rx) = tokio::sync::mpsc::channel(4);
    let raw_target = if workspace.trim().is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(&workspace))
    };
    let resolved = match resolve_index_target(raw_target) {
        Ok(resolved) => resolved,
        Err(err) => {
            let msg = err.to_string();
            {
                let mut system_guard = state.system.write().await;
                system_guard.record_parse_failure(std::path::PathBuf::from(&workspace), msg.clone());
            }
            add_msg_shortcut(&msg).await;
            event_bus.send(AppEvent::Error(ErrorEvent {
                message: msg,
                severity: ErrorSeverity::Error,
            }));
            return;
        }
    };

    if needs_parse {
        match run_parse_resolved(Arc::clone(&state.db), &resolved) {
            Ok(_) => {
                tracing::info!(
                    "Parse of target {} successful",
                    resolved.workspace_root.display()
                );
            }
            Err(e) => {
                let msg = format_parse_failure(&resolved.focused_root, &e);
                {
                    let mut system_guard = state.system.write().await;
                    system_guard
                        .record_parse_failure(resolved.requested_path.clone(), msg.clone());
                }
                event_bus.send(AppEvent::Error(ErrorEvent {
                    message: msg,
                    severity: ErrorSeverity::Error,
                }));
                tracing::info!("Failure parsing directory from IndexWorkspace event: {}", e);
                return;
            }
        }
    }

    let policy = {
        let mut system_guard = state.system.write().await;
        system_guard.set_loaded_workspace(
            resolved.workspace_root.clone(),
            resolved.member_roots.clone(),
            Some(resolved.focused_root.clone()),
        );
        system_guard.record_parse_success();
        system_guard.derive_path_policy(&[])
    };
    if let Some(policy) = policy {
        state
            .io_handle
            .update_roots(Some(policy.roots), Some(policy.symlink_policy))
            .await;
    }
    tracing::info!("end parse");

    add_msg_immediate(
        state,
        event_bus,
        Uuid::new_v4(),
        "Indexing...".to_string(),
        crate::chat_history::MessageKind::SysInfo,
    )
    .await;

    maybe_delay_indexing_for_test().await;

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
                let err_msg = e.to_string();
                add_msg_shortcut(&err_msg).await;
                tracing::warn!("Sending Indexing Failed with error message: {}", err_msg);
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
