use std::sync::Arc;

use crate::system::SystemEvent;
use crate::{EventBus, RagEvent};
use tokio::sync::mpsc;

use super::commands::StateCommand;
use super::core::AppState;
use super::handlers;
use crate::chat_history::MessageKind;
use uuid::Uuid;

pub async fn state_manager(
    state: Arc<AppState>,
    mut cmd_rx: mpsc::Receiver<StateCommand>,
    event_bus: Arc<EventBus>,
    context_tx: mpsc::Sender<RagEvent>,
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

    while let Some(cmd) = cmd_rx.recv().await {
        let span = tracing::debug_span!("processing", cmd = %cmd.discriminant());
        let _enter = span.enter();

        match cmd {
            StateCommand::UpdateMessage { id, update } => {
                handlers::chat::update_message(&state, &event_bus, id, update).await;
            }
            StateCommand::DeleteMessage { id } => {
                handlers::chat::delete_message(&state, &event_bus, id).await;
            }
            StateCommand::DeleteNode { id } => {
                {
                    let mut guard = state.chat.0.write().await;
                    let _ = guard.delete_node(id);
                }
                // No explicit message; UI will redraw on next tick
            }
            StateCommand::AddUserMessage {
                content,
                new_msg_id,
                completion_tx,
            } => {
                handlers::chat::add_user_message(
                    &state,
                    &event_bus,
                    new_msg_id,
                    content,
                    completion_tx,
                )
                .await;
            }
            StateCommand::AddMessage {
                parent_id,
                child_id,
                content,
                kind,
                target: _,
            } => {
                handlers::chat::add_message(&state, &event_bus, parent_id, child_id, content, kind)
                    .await;
            }
            StateCommand::AddMessageImmediate {
                msg,
                kind,
                new_msg_id,
            } => {
                handlers::chat::add_msg_immediate(&state, &event_bus, new_msg_id, msg, kind).await;
            }
            StateCommand::PruneHistory { max_messages: _ } => {
                handlers::chat::prune_history().await;
            }
            StateCommand::NavigateList { direction } => {
                handlers::chat::navigate_list(&state, &event_bus, direction).await;
            }
            StateCommand::CreateAssistantMessage {
                parent_id,
                responder,
            } => {
                handlers::chat::create_assistant_message(&state, &event_bus, parent_id, responder)
                    .await;
            }

            StateCommand::IndexWorkspace {
                workspace,
                needs_parse,
            } => {
                handlers::indexing::index_workspace(&state, &event_bus, workspace, needs_parse)
                    .await;
            }
            StateCommand::PauseIndexing => handlers::indexing::pause(&state).await,
            StateCommand::ResumeIndexing => handlers::indexing::resume(&state).await,
            StateCommand::CancelIndexing => handlers::indexing::cancel(&state).await,

            StateCommand::SaveState => {
                handlers::session::save_state(&state, &event_bus).await;
            }

            StateCommand::UpdateDatabase => {
                handlers::db::update_database(&state, &event_bus).await;
            }

            StateCommand::EmbedMessage {
                new_msg_id,
                completion_rx,
                scan_rx,
            } => {
                handlers::rag::process_with_rag(
                    &state,
                    &event_bus,
                    scan_rx,
                    new_msg_id,
                    completion_rx,
                )
                .await;
                // handlers::embedding::handle_embed_message(&state, &context_tx, new_msg_id, completion_rx, scan_rx).await;
            }
            // StateCommand::ProcessWithRag { user_query, strategy, budget } => {
            // }
            StateCommand::SwitchModel { alias_or_id } => {
                handlers::model::switch_model(&state, &event_bus, alias_or_id).await;
            }

            StateCommand::SetEditingPreviewMode { mode } => {
                {
                    let mut cfg = state.config.write().await;
                    cfg.editing.preview_mode = mode;
                }
                let mode_label = match mode {
                    crate::app_state::core::PreviewMode::CodeBlock => "codeblock",
                    crate::app_state::core::PreviewMode::Diff => "diff",
                };
                handlers::chat::add_msg_immediate(
                    &state,
                    &event_bus,
                    Uuid::new_v4(),
                    format!("Edit preview mode set to {}", mode_label),
                    MessageKind::SysInfo,
                )
                .await;
            }
            StateCommand::SetEditingMaxPreviewLines { lines } => {
                {
                    let mut cfg = state.config.write().await;
                    cfg.editing.max_preview_lines = lines;
                }
                handlers::chat::add_msg_immediate(
                    &state,
                    &event_bus,
                    Uuid::new_v4(),
                    format!("Edit preview lines set to {}", lines),
                    MessageKind::SysInfo,
                )
                .await;
            }
            StateCommand::SetEditingAutoConfirm { enabled } => {
                {
                    let mut cfg = state.config.write().await;
                    cfg.editing.auto_confirm_edits = enabled;
                }
                handlers::chat::add_msg_immediate(
                    &state,
                    &event_bus,
                    Uuid::new_v4(),
                    if enabled {
                        "Auto-approval of edits enabled".to_string()
                    } else {
                        "Auto-approval of edits disabled".to_string()
                    },
                    MessageKind::SysInfo,
                )
                .await;
            }

            StateCommand::WriteQuery {
                query_name: _,
                query_content,
            } => {
                handlers::db::write_query(&state, query_content).await;
            }
            StateCommand::ReadQuery {
                query_name,
                file_name,
            } => {
                handlers::db::read_query(&event_bus, query_name, file_name).await;
            }
            StateCommand::SaveDb => {
                handlers::db::save_db(&state, &event_bus).await;
            }
            StateCommand::BatchPromptSearch {
                prompt_file,
                out_file,
                max_hits,
                threshold,
            } => {
                handlers::db::batch_prompt_search(
                    &state,
                    prompt_file,
                    out_file,
                    max_hits,
                    threshold,
                    &event_bus,
                )
                .await;
            }
            StateCommand::LoadDb { crate_name } => {
                handlers::db::load_db(&state, &event_bus, crate_name).await;
            }
            StateCommand::ScanForChange { scan_tx } => {
                handlers::db::scan_for_change(&state, &event_bus, scan_tx).await;
            }

            StateCommand::Bm25Rebuild => handlers::rag::bm25_rebuild(&state, &event_bus).await,
            StateCommand::Bm25Search { query, top_k } => {
                handlers::rag::bm25_search(&state, &event_bus, query, top_k).await
            }
            StateCommand::HybridSearch { query, top_k } => {
                handlers::rag::hybrid_search(&state, &event_bus, query, top_k).await
            }
            StateCommand::RagBm25Status => handlers::rag::bm25_status(&state, &event_bus).await,
            StateCommand::RagBm25Save { path } => {
                handlers::rag::bm25_save(&state, &event_bus, path).await
            }
            StateCommand::RagBm25Load { path } => {
                handlers::rag::bm25_load(&state, &event_bus, path).await
            }
            StateCommand::RagSparseSearch {
                req_id,
                query,
                top_k,
                strict,
            } => {
                handlers::rag::sparse_search(&state, &event_bus, req_id, query, top_k, strict).await
            }
            StateCommand::RagDenseSearch {
                req_id,
                query,
                top_k,
            } => handlers::rag::dense_search(&state, &event_bus, req_id, query, top_k).await,
            StateCommand::RagAssembleContext {
                req_id,
                user_query,
                top_k,
                budget,
                strategy,
            } => {
                handlers::rag::assemble_context(
                    &state, &event_bus, req_id, user_query, top_k, &budget, strategy,
                )
                .await
            }
            StateCommand::ApproveEdits { request_id } => {
                handlers::rag::approve_edits(&state, &event_bus, request_id).await;
            }
            StateCommand::DenyEdits { request_id } => {
                handlers::rag::deny_edits(&state, &event_bus, request_id).await;
            }

            _ => {}
        };
    }
}
