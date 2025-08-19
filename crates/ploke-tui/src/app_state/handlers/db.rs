use std::sync::Arc;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::{AppEvent, EventBus, app_state::database, error::ErrorExt as _};

use crate::AppState;

pub async fn update_database(state: &Arc<AppState>, event_bus: &Arc<EventBus>) {
    use ploke_db::{NodeType, create_index_warn, replace_index_warn};
    use tokio::time;

    let start = time::Instant::now();

    super::chat::add_msg_immediate(
        state,
        event_bus,
        Uuid::new_v4(),
        "Indexing HNSW...".to_string(),
        crate::chat_history::MessageKind::SysInfo,
    )
    .await;

    for ty in NodeType::primary_nodes() {
        match create_index_warn(&state.db, ty) {
            Ok(_) => {
                tracing::info!(
                    "Database index updated by create_index_warn for rel: {}",
                    ty.relation_str()
                );
            }
            Err(_) => {
                match replace_index_warn(&state.db, ty) {
                    Ok(_) => {
                        tracing::info!(
                            "Database index updated by replace_index_warn for rel: {}",
                            ty.relation_str()
                        );
                    }
                    Err(_) => {
                        tracing::warn!("The attempt to replace the index at the database failed")
                    }
                }
                tracing::warn!("The attempt to create the index at the database failed")
            }
        }
    }
    let after = time::Instant::now();
    let msg = format!("..finished in {}", after.duration_since(start).as_millis());

    super::chat::add_msg_immediate(
        state,
        event_bus,
        Uuid::new_v4(),
        msg,
        crate::chat_history::MessageKind::SysInfo,
    )
    .await;
}

pub async fn write_query(state: &Arc<AppState>, query_content: String) {
    database::write_query(state, query_content).await;
}

pub async fn read_query(event_bus: &Arc<EventBus>, query_name: String, file_name: String) {
    let _ = event_bus
        .realtime_tx
        .send(AppEvent::System(crate::system::SystemEvent::ReadQuery {
            query_name: query_name.clone(),
            file_name: file_name.clone(),
        }))
        .inspect_err(|e| tracing::warn!("Error forwarding event: {e:?}"));
    let _ = event_bus
        .background_tx
        .send(AppEvent::System(crate::system::SystemEvent::ReadQuery {
            query_name,
            file_name,
        }))
        .inspect_err(|e| tracing::warn!("Error forwarding event: {e:?}"));
}

pub async fn save_db(state: &Arc<AppState>, event_bus: &Arc<EventBus>) {
    use database::save_db;
    if let std::ops::ControlFlow::Break(_) = save_db(state, event_bus).await {
        return;
    }
}

pub async fn batch_prompt_search(
    state: &Arc<AppState>,
    prompt_file: String,
    out_file: String,
    max_hits: Option<usize>,
    threshold: Option<f32>,
    event_bus: &Arc<EventBus>,
) {
    match database::batch_prompt_search(state, prompt_file, out_file, max_hits, threshold).await {
        Ok(embed_data) => {
            tracing::info!(
                "Batch prompt search succeeded with {} results.",
                embed_data.len()
            );
        }
        Err(e) => {
            tracing::error!(
                error = %e,
                error_chain = ?e.chain().collect::<Vec<_>>(),
                "Batch prompt search failed"
            );
        }
    }
}

pub async fn load_db(state: &Arc<AppState>, event_bus: &Arc<EventBus>, crate_name: String) {
    if let Err(e) = database::load_db(state, event_bus, crate_name).await {
        match e {
            ploke_error::Error::Fatal(_) => e.emit_fatal(),
            ploke_error::Error::Warning(_) | ploke_error::Error::Internal(_) => e.emit_warning(),
            _ => {
                todo!("These should never happen.")
            }
        }
    }
}

pub async fn scan_for_change(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    scan_tx: oneshot::Sender<Option<Vec<std::path::PathBuf>>>,
) {
    let _ = database::scan_for_change(state, event_bus, scan_tx)
        .await
        .inspect_err(|e| {
            e.emit_error();
            tracing::error!("Error in ScanForChange:\n{e}");
        });
}
