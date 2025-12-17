use std::ops::ControlFlow;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::{RagEvent, app_state::AppState, chat_history::Message, error::ResultExt as _};
use ploke_db::{NodeType, search_similar};

/// Handles embedding of a user message and subsequent similarity search.
///
/// # Error Handling
/// This function gracefully handles all errors to prevent panics in spawned tasks,
/// which would otherwise trigger the global panic hook and corrupt terminal state.
/// See regression test `test_embed_message_graceful_error_handling` for validation.
///
/// # Terminal State Corruption Issue (Fixed)
/// Previously, `.expect()` calls in this function could panic when:
/// - Embedding generation failed (e.g., model not available, network error)
/// - Empty embedding results were returned
///
/// These panics would trigger `ratatui::restore()` via the global panic hook,
/// causing the TUI to exit raw mode while still running, resulting in cargo
/// warnings and LLM messages being written over each other in the terminal.
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

            // CRITICAL: Use proper error handling instead of .expect() to prevent panics
            // that would corrupt terminal state via the global panic hook.
            let temp_embed = match state
                .embedder
                .generate_embeddings(vec![last_user_msg])
                .await
            {
                Ok(embeddings) => embeddings,
                Err(e) => {
                    tracing::error!(
                        "Failed to generate embeddings for user message: {}. \
                         This may indicate an embedding model configuration issue.",
                        e
                    );
                    // Early return without panicking - terminal state preserved
                    return;
                }
            };
            drop(chat_guard);

            // CRITICAL: Handle empty embedding results gracefully
            let embeddings = match temp_embed.into_iter().next() {
                Some(emb) => emb,
                None => {
                    tracing::error!(
                        "Embedding generation returned no results. \
                         Check embedding model configuration."
                    );
                    // Early return without panicking - terminal state preserved
                    return;
                }
            };
            tracing::info!("Finish embedding user message");

            tracing::info!("Waiting to finish processing updates to files, if any");
            if let ControlFlow::Break(_) = wait_on_oneshot(new_msg_id, scan_rx).await {
                return;
            }
            tracing::info!("Finished waiting on parsing target crate");

            if let Err(e) =
                embedding_search_similar(state, context_tx, new_msg_id, embeddings).await
            {
                tracing::error!("error during embedding search: {}", e);
            };
        }
        Ok(None) => {
            tracing::warn!("Could not retrieve last user message from the conversation history");
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
    let ty_embed_data =
        search_similar(&state.db, embeddings, 100, 200, NodeType::Function).emit_error()?;
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

pub(crate) async fn wait_on_oneshot<T>(
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
