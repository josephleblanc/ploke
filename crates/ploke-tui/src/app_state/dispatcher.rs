use std::str::FromStr;
use std::sync::Arc;

use crate::llm::ProviderSlug;
use crate::llm::registry::user_prefs::{ModelPrefs, RegistryPrefs};
use crate::llm::router_only::RouterVariants;
use crate::llm::{EndpointKey, ModelId, ProviderKey};
use crate::rag::context::process_with_rag;
use crate::{EventBus, RagEvent, rag};
use serde::Deserialize;
use tokio::sync::mpsc;

use super::commands::StateCommand;
use super::core::AppState;
use super::events::SystemEvent;
use super::{database, handlers};
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
                completion_tx,
                new_user_msg_id,
            } => {
                handlers::chat::add_user_message(
                    &state,
                    &event_bus,
                    new_user_msg_id,
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
            StateCommand::AddMessageTool { msg, kind, new_msg_id, tool_call_id } => {
                handlers::chat::add_tool_msg_immediate(&state, &event_bus, new_msg_id, msg, tool_call_id ).await;
            }
            StateCommand::PruneHistory { max_messages: _ } => {
                handlers::chat::prune_history().await;
            }
            StateCommand::NavigateList { direction } => {
                handlers::chat::navigate_list(&state, &event_bus, direction).await;
            }
            StateCommand::CreateAssistantMessage {
                new_assistant_msg_id,
                parent_id,
                responder,
            } => {
                handlers::chat::create_assistant_message(
                    &state,
                    &event_bus,
                    parent_id,
                    responder,
                    new_assistant_msg_id,
                )
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
                process_with_rag(&state, &event_bus, scan_rx, new_msg_id, completion_rx).await;
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
                database::save_db(&state, &event_bus).await;
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

            StateCommand::Bm25Rebuild => rag::search::bm25_rebuild(&state, &event_bus).await,
            StateCommand::Bm25Search { query, top_k } => {
                rag::search::bm25_search(&state, &event_bus, query, top_k).await
            }
            StateCommand::HybridSearch { query, top_k } => {
                rag::search::hybrid_search(&state, &event_bus, query, top_k).await
            }
            StateCommand::RagBm25Status => rag::search::bm25_status(&state, &event_bus).await,
            StateCommand::RagBm25Save { path } => {
                rag::search::bm25_save(&state, &event_bus, path).await
            }
            StateCommand::RagBm25Load { path } => {
                rag::search::bm25_load(&state, &event_bus, path).await
            }
            StateCommand::RagSparseSearch {
                req_id,
                query,
                top_k,
                strict,
            } => rag::search::sparse_search(&state, &event_bus, req_id, query, top_k, strict).await,
            StateCommand::RagDenseSearch {
                req_id,
                query,
                top_k,
            } => rag::search::dense_search(&state, &event_bus, req_id, query, top_k).await,
            // NOTE: I think this is no longer being used. Commenting out to look for errors if
            // absent, delete later.
            // - 2025-08-28
            //
            // StateCommand::RagAssembleContext {
            //     req_id,
            //     user_query,
            //     top_k,
            //     budget,
            //     strategy,
            // } => {
            //     rag::context::assemble_context(
            //         &state, &event_bus, req_id, user_query, top_k, &budget, strategy,
            //     )
            //     .await
            // }
            StateCommand::ApproveEdits { request_id } => {
                rag::editing::approve_edits(&state, &event_bus, request_id).await;
            }
            StateCommand::DenyEdits { request_id } => {
                rag::editing::deny_edits(&state, &event_bus, request_id).await;
            }
            StateCommand::ApproveCreations { request_id } => {
                rag::editing::approve_creations(&state, &event_bus, request_id).await;
            }
            StateCommand::DenyCreations { request_id } => {
                rag::editing::deny_creations(&state, &event_bus, request_id).await;
            }
            StateCommand::SelectModelProvider {
                model_id_string,
                provider_key,
            } => {
                // Check registry for model, then
                // Update registry prefs and active runtime selection to match user's choice.
                let mut cfg = state.config.write().await;
                let reg = &mut cfg.model_registry;
                let model_id = match ModelId::from_str(&model_id_string) {
                    Ok(m) => m,
                    Err(e) => {
                        let msg = format!( "Invalid model id: {}, expected `{{auther}}/{{model}}:{{variant}}` where `:{{variant}}` is optional", model_id_string );
                        add_msg_shortcut(&msg).await;
                        continue
                    }
                };

                // Ensure a ModelPrefs entry exists for this model key
                reg.models
                    .entry(model_id.clone().key)
                    .or_insert_with(|| ModelPrefs {
                        model_key: model_id.clone().key,
                        ..Default::default()
                    });

                // Ensure OpenRouter is allowed (for now we only support OpenRouter)
                let mp = reg
                    .models
                    .get_mut(&model_id.clone().key)
                    .expect("entry ensured above");
                if !mp
                    .allowed_routers
                    .iter()
                    .any(|r| matches!(r, RouterVariants::OpenRouter(_)))
                {
                    mp.allowed_routers.push(RouterVariants::OpenRouter(
                        crate::llm::router_only::openrouter::OpenRouter,
                    ));
                }

                let msg = if let Some(provider) = provider_key {
                    // Add/update selected endpoint preference
                    let ModelId { key, variant } = model_id.clone();
                    let ek = EndpointKey {
                        model: key,
                        provider: provider.clone(),
                        variant,
                    };
                    if !mp.selected_endpoints.iter().any(|e| e == &ek) {
                        mp.selected_endpoints.push(ek);
                    }
                    // otherwise selected model without provider, which is fine.
                    format!(
                        "Switched active model to {} via provider {}",
                        model_id, provider.slug.as_str() 
                    )
                } else {
                    format!("Switched active model to {} with auto provider selection", model_id)
                };

                // Inform the user and update the UI via events

                // Set active runtime model to the chosen id (includes optional variant)
                cfg.active_model = model_id.clone();
                handlers::chat::add_msg_immediate(
                    &state,
                    &event_bus,
                    Uuid::new_v4(),
                    msg,
                    MessageKind::SysInfo,
                )
                .await;

                event_bus.send(crate::AppEvent::System(SystemEvent::ModelSwitched(
                    model_id,
                )));
            }
            StateCommand::DecrementChatTtl => {
                let mut chat_history = state.chat.write().await;
                chat_history.decrement_ttl();
            }

            _ => {}
        };
    }
}
