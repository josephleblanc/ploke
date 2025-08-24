use crate::{app_state::handlers::chat, chat_history::MessageKind, llm::ToolEvent};

use super::*;
pub async fn approve_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    use crate::app_state::core::EditProposalStatus;
    let mut reg = state.proposals.write().await;
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate(state, event_bus, Uuid::new_v4(), msg, MessageKind::SysInfo).await
    };
    let Some(mut proposal) = reg.get(&request_id).cloned() else {
        let msg = format!(
            "No staged edit proposal found for request_id {}",
            request_id
        );
        add_msg_imm(msg).await;
        return;
    };

    // Idempotency checks
    match proposal.status {
        EditProposalStatus::Pending => { /* ok */ }
        EditProposalStatus::Applied => {
            let msg = format!("Edits already applied for request_id {}", request_id);
            add_msg_imm(msg).await;
            return;
        }
        EditProposalStatus::Denied => {
            let msg = format!("Edits already denied for request_id {}", request_id);
            add_msg_imm(msg).await;
            return;
        }
        EditProposalStatus::Approved => {
            tracing::debug!("Edit proposal appoved, beginning edit");
            // Treat as attempting to apply again
        }
        EditProposalStatus::Failed(_) => {
            tracing::debug!("Edit proposal failed, attempting edit again");
            // Allow re-apply attempt
        }
    }

    // Apply edits via IoManagerHandle
    let file_paths = proposal.files.clone();
    match state
        .io_handle
        .write_snippets_batch(proposal.edits.clone())
        .await
    {
        Ok(results) => {
            let applied = results.iter().filter(|r| r.is_ok()).count();
            let results_json: Vec<serde_json::Value> = results
                .into_iter()
                .zip(file_paths.into_iter())
                .map(|(res, path)| match res {
                    Ok(write_res) => serde_json::json!({
                        "file_path": path.display().to_string(),
                        "new_file_hash": write_res.new_file_hash.0.to_string(),
                    }),
                    Err(err) => serde_json::json!({
                        "file_path": path.display().to_string(),
                        "error": err.to_string(),
                    }),
                })
                .collect();

            let content = serde_json::json!({
                "ok": applied > 0,
                "applied": applied,
                "results": results_json
            })
            .to_string();

            // Update state: mark applied
            proposal.status = EditProposalStatus::Applied;
            reg.insert(request_id, proposal);

            // Bridge: mark tool call completed
            let parent_id_val = reg
                .get(&request_id)
                .map(|p| p.parent_id)
                .unwrap_or_default();
            let call_id_val = reg
                .get(&request_id)
                .map(|p| p.call_id.clone())
                .unwrap_or_default();
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
                    content: content.clone(),
                }));
            event_bus.send(AppEvent::LlmTool(ToolEvent::Completed {
                request_id,
                parent_id: parent_id_val,
                call_id: call_id_val,
                content: content.clone(),
            }));

            chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Applied edits for request_id {}", request_id),
                MessageKind::SysInfo,
            )
            .await;
        }
        Err(e) => {
            proposal.status = EditProposalStatus::Failed(e.to_string());
            reg.insert(request_id, proposal);

            let parent_id_val = reg
                .get(&request_id)
                .map(|p| p.parent_id)
                .unwrap_or_default();
            let call_id_val = reg
                .get(&request_id)
                .map(|p| p.call_id.clone())
                .unwrap_or_default();
            let err_str = format!("Failed to apply edits: {}", e);
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
                    error: err_str.clone(),
                }));
            event_bus.send(AppEvent::LlmTool(ToolEvent::Failed {
                request_id,
                parent_id: parent_id_val,
                call_id: call_id_val,
                error: err_str.clone(),
            }));

            let msg = format!("Failed to apply edits for request_id {}: {}", request_id, e);
            add_msg_imm(msg).await;
        }
    }
}

pub async fn deny_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    use crate::app_state::core::EditProposalStatus;
    let mut reg = state.proposals.write().await;
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate(state, event_bus, Uuid::new_v4(), msg, MessageKind::SysInfo).await
    };

    let Some(mut proposal) = reg.get(&request_id).cloned() else {
        let msg = format!(
            "No staged edit proposal found for request_id {}",
            request_id
        );
        add_msg_imm(msg).await;
        return;
    };

    match proposal.status {
        EditProposalStatus::Pending
        | EditProposalStatus::Approved
        | EditProposalStatus::Failed(_) => {
            proposal.status = EditProposalStatus::Denied;
            reg.insert(request_id, proposal);

            // Bridge: mark tool call failed with denial
            let parent_id_val = reg
                .get(&request_id)
                .map(|p| p.parent_id)
                .unwrap_or_default();
            let call_id_val = reg
                .get(&request_id)
                .map(|p| p.call_id.clone())
                .unwrap_or_default();
            let err_msg = "Edit proposal denied by user".to_string();
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
                    error: err_msg.clone(),
                }));
            event_bus.send(AppEvent::LlmTool(ToolEvent::Failed {
                request_id,
                parent_id: parent_id_val,
                call_id: call_id_val,
                error: err_msg.clone(),
            }));

            let msg = format!("Denied edits for request_id {}", request_id);
            add_msg_imm(msg).await;
        }
        EditProposalStatus::Denied => {
            let msg = format!("Edits already denied for request_id {}", request_id);
            add_msg_imm(msg).await;
        }
        EditProposalStatus::Applied => {
            let msg = format!("Edits already applied for request_id {}", request_id);
            add_msg_imm(msg).await;
        }
    }
}
