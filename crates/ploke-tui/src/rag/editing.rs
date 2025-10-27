use crate::{app_state::handlers::chat, chat_history::MessageKind};

use super::*;
pub async fn approve_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    use crate::app_state::core::EditProposalStatus;
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate(state, event_bus, Uuid::new_v4(), msg, MessageKind::SysInfo).await
    };
    let reg = state.proposals.write().await;
    let Some(mut proposal) = reg.get(&request_id).cloned() else {
        let msg = format!(
            "No staged edit proposal found for request_id {}",
            request_id
        );
        drop(reg);
        add_msg_imm(msg).await;
        return;
    };
    drop(reg);

    // Idempotency checks (without holding lock)
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
        }
        EditProposalStatus::Failed(_) => {
            tracing::debug!("Edit proposal failed, attempting edit again");
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
            let parent_id_val = proposal.parent_id;
            let call_id_val = proposal.call_id.clone();
            let mut reg = state.proposals.write().await;
            reg.insert(request_id, proposal);
            drop(reg);
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
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

            // Persist proposals (best-effort)
            crate::app_state::handlers::proposals::save_proposals(state).await;

            // Post-apply: trigger a rescan to refresh indexes
            let (scan_tx, scan_rx) = tokio::sync::oneshot::channel();
            tokio::spawn({
                let state = Arc::clone(state);
                let event_bus = Arc::clone(event_bus);
                async move {
                    crate::app_state::handlers::db::scan_for_change(&state, &event_bus, scan_tx).await;
                    // We don't need scan_rx result here; fire-and-forget.
                    let _ = scan_rx;
                }
            });
            // Surface a brief SysInfo so users see that a rescan has been scheduled
            chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                "Scheduled rescan of workspace after applying edits".to_string(),
                MessageKind::SysInfo,
            )
            .await;
        }
        Err(e) => {
            proposal.status = EditProposalStatus::Failed(e.to_string());
            let parent_id_val = proposal.parent_id;
            let call_id_val = proposal.call_id.clone();
            let mut reg = state.proposals.write().await;
            reg.insert(request_id, proposal);
            drop(reg);
            let err_str = format!("Failed to apply edits: {}", e);
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
                    error: err_str.clone(),
                }));

            let msg = format!("Failed to apply edits for request_id {}: {}", request_id, e);
            add_msg_imm(msg).await;
            crate::app_state::handlers::proposals::save_proposals(state).await;
        }
    }
}

pub async fn deny_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    use crate::app_state::core::EditProposalStatus;
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate(state, event_bus, Uuid::new_v4(), msg, MessageKind::SysInfo).await
    };
    let reg = state.proposals.write().await;
    let Some(mut proposal) = reg.get(&request_id).cloned() else {
        let msg = format!(
            "No staged edit proposal found for request_id {}",
            request_id
        );
        drop(reg);
        add_msg_imm(msg).await;
        return;
    };
    drop(reg);

    match proposal.status {
        EditProposalStatus::Pending
        | EditProposalStatus::Approved
        | EditProposalStatus::Failed(_) => {
            proposal.status = EditProposalStatus::Denied;
            let parent_id_val = proposal.parent_id;
            let call_id_val = proposal.call_id.clone();
            let mut reg = state.proposals.write().await;
            reg.insert(request_id, proposal);
            drop(reg);

            // Bridge: mark tool call failed with denial
            let err_msg = "Edit proposal denied by user".to_string();
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
                    error: err_msg.clone(),
                }));

            let msg = format!("Denied edits for request_id {}", request_id);
            add_msg_imm(msg).await;
            crate::app_state::handlers::proposals::save_proposals(state).await;
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

pub async fn approve_creations(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    request_id: Uuid,
) {
    use crate::app_state::core::EditProposalStatus;
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate(state, event_bus, Uuid::new_v4(), msg, MessageKind::SysInfo).await
    };
    let reg = state.create_proposals.write().await;
    let Some(mut proposal) = reg.get(&request_id).cloned() else {
        let msg = format!(
            "No staged create-file proposal found for request_id {}",
            request_id
        );
        drop(reg);
        add_msg_imm(msg).await;
        return;
    };
    drop(reg);

    match proposal.status {
        EditProposalStatus::Pending => {}
        EditProposalStatus::Applied => {
            let msg = format!("Creations already applied for request_id {}", request_id);
            add_msg_imm(msg).await;
            return;
        }
        EditProposalStatus::Denied => {
            let msg = format!("Creations already denied for request_id {}", request_id);
            add_msg_imm(msg).await;
            return;
        }
        EditProposalStatus::Approved => {
            tracing::debug!("Create-file proposal approved, applying");
        }
        EditProposalStatus::Failed(_) => {
            tracing::debug!("Create-file proposal previously failed; retrying apply");
        }
    }

    // Apply creations via IoManagerHandle
    let file_paths = proposal.files.clone();
    let mut applied = 0usize;
    let mut results_json: Vec<serde_json::Value> = Vec::with_capacity(proposal.creates.len());
    for req in proposal.creates.clone() {
        match state.io_handle.create_file(req.clone()).await {
            Ok(Ok(res)) => {
                applied += 1;
                results_json.push(serde_json::json!({
                    "file_path": req.file_path.display().to_string(),
                    "new_file_hash": res.new_file_hash.0.to_string(),
                }));
            }
            Ok(Err(err)) => {
                results_json.push(serde_json::json!({
                    "file_path": req.file_path.display().to_string(),
                    "error": err.to_string(),
                }));
            }
            Err(e) => {
                results_json.push(serde_json::json!({
                    "file_path": req.file_path.display().to_string(),
                    "error": e.to_string(),
                }));
            }
        }
    }

    let content = serde_json::json!({
        "ok": applied > 0,
        "applied": applied,
        "results": results_json
    })
    .to_string();

    proposal.status = EditProposalStatus::Applied;
    let parent_id_val = proposal.parent_id;
    let call_id_val = proposal.call_id.clone();
    let mut reg = state.create_proposals.write().await;
    reg.insert(request_id, proposal);
    drop(reg);

    let _ = event_bus
        .realtime_tx
        .send(AppEvent::System(SystemEvent::ToolCallCompleted {
            request_id,
            parent_id: parent_id_val,
            call_id: call_id_val,
            content: content.clone(),
        }));

    chat::add_msg_immediate(
        state,
        event_bus,
        Uuid::new_v4(),
        format!("Applied file creations for request_id {}", request_id),
        MessageKind::SysInfo,
    )
    .await;

    // Persist proposals (best-effort)
    crate::app_state::handlers::proposals::save_create_proposals(state).await;

    // Post-apply: trigger a rescan to refresh indexes
    let (scan_tx, scan_rx) = tokio::sync::oneshot::channel();
    tokio::spawn({
        let state = Arc::clone(state);
        let event_bus = Arc::clone(event_bus);
        async move {
            crate::app_state::handlers::db::scan_for_change(&state, &event_bus, scan_tx).await;
            let _ = scan_rx.await.inspect_err(|e| tracing::error!(scan_error = ?e));
        }
    });
    chat::add_msg_immediate(
        state,
        event_bus,
        Uuid::new_v4(),
        "Scheduled rescan of workspace after creating files".to_string(),
        MessageKind::SysInfo,
    )
    .await;
}

pub async fn deny_creations(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    request_id: Uuid,
) {
    use crate::app_state::core::EditProposalStatus;
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate(state, event_bus, Uuid::new_v4(), msg, MessageKind::SysInfo).await
    };

    let reg = state.create_proposals.write().await;
    let Some(mut proposal) = reg.get(&request_id).cloned() else {
        let msg = format!(
            "No staged create-file proposal found for request_id {}",
            request_id
        );
        drop(reg);
        add_msg_imm(msg).await;
        return;
    };
    drop(reg);

    match proposal.status {
        EditProposalStatus::Pending | EditProposalStatus::Approved | EditProposalStatus::Failed(_) => {
            proposal.status = EditProposalStatus::Denied;
            let parent_id_val = proposal.parent_id;
            let call_id_val = proposal.call_id.clone();
            let mut regw = state.create_proposals.write().await;
            regw.insert(request_id, proposal);
            drop(regw);

            // Bridge: mark tool call failed with denial
            let err_msg = "Create-file proposal denied by user".to_string();
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
                    error: err_msg.clone(),
                }));

            let msg = format!("Denied file creations for request_id {}", request_id);
            add_msg_imm(msg).await;
            crate::app_state::handlers::proposals::save_create_proposals(state).await;
        }
        EditProposalStatus::Denied => {
            let msg = format!("Creations already denied for request_id {}", request_id);
            add_msg_imm(msg).await;
        }
        EditProposalStatus::Applied => {
            let msg = format!("Creations already applied for request_id {}", request_id);
            add_msg_imm(msg).await;
        }
    }
}
