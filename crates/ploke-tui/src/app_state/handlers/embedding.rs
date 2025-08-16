use std::ops::ControlFlow;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::chat_history::Message;
use ploke_db::{search_similar, NodeType};

use super::super::core::AppState;

pub async fn handle_embed_message(
    state: &Arc<AppState>,
    context_tx: &mpsc::Sender<RagEvent>,
    new_msg_id: Uuid,
    completion_rx: oneshot::Receiver<()>,
    scan_rx: oneshot::Receiver<Option<Vec<std::path::PathBuf>>>,
) {
    if let ControlFlow::Break(_) = wait_on_oneshot(new_msg_id, completion_rx).await {
        return;
    }
    let chat_guard = state.chat.0.read().await;
    match chat_guard.last_user_msg() {
        Ok(Some((_last_usr_msg_id, last_user_msg))) => {
            tracing::info!("Start embedding user message: {}", last_user_msg);
            let temp_embed = state
                .embedder
                .generate_embeddings(vec![last_user_msg])
                .await
                .expect("Error while generating embedding of user message");
            drop(chat_guard);
            let embeddings = temp_embed
                .into_iter()
                .next()
                .expect("No results from user message embedding generation");
            tracing::info!("Finish embedding user message");

            tracing::info!("Waiting to finish processing updates to files, if any");
            if let ControlFlow::Break(_) = wait_on_oneshot(new_msg_id, scan_rx).await {
                return;
            }
            tracing::info!("Finished waiting on parsing target crate");

            if let Err(e) = embedding_search_similar(state, context_tx, new_msg_id, embeddings).await {
                tracing::error!("error during embedding search: {}", e);
            };
        }
        Ok(None) => {
            tracing::warn!("Could not retreive last user message from the conversation history");
        }
        Err(e) => {
            tracing::error!("Error accessing last user message: {:#}", e);
        }
    }
}

async fn embedding_search_similar(
    state: &Arc<AppState>,
    context_tx: &mpsc::Sender<RagEvent>,
    new_msg_id: Uuid,
    embeddings: Vec<f32>,
) -> color_eyre::Result<()> {
    let ty_embed_data = search_similar(&state.db, embeddings, 100, 200, NodeType::Function).emit_error()?;
    tracing::info!("search_similar Success! with result {:?}", ty_embed_data);

    let snippets = state
        .io_handle
        .get_snippets_batch(ty_embed_data.v)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| r.ok())
        .collect::<Vec<String>>();

    context_tx
        .send(RagEvent::ContextSnippets(new_msg_id, snippets))
        .await?;

    let messages: Vec<Message> = state.chat.0.read().await.clone_current_path_conv();

    context_tx
        .send(RagEvent::UserMessages(new_msg_id, messages))
        .await?;
    context_tx
        .send(RagEvent::ConstructContext(new_msg_id))
        .await?;
    Ok(())
}

async fn wait_on_oneshot<T>(
    new_msg_id: Uuid,
    completion_rx: oneshot::Receiver<T>,
) -> ControlFlow<()> {
    match completion_rx.await {
        Ok(_) => {
            tracing::trace!("UserMessage received new_msg_id: {}", new_msg_id)
        }
        Err(_e) => {
            tracing::warn!(
                "SendUserMessage dropped before EmbedMessage process received it for new_msg_id: {}",
                new_msg_id
            );
            return ControlFlow::Break(());
        }
    }
    ControlFlow::Continue(())
}
