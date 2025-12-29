use std::collections::HashMap;

use crate::{
    app_state::{core::EditProposalStatus, handlers::chat},
    chat_history::MessageKind,
    tools::{ToolError, ToolErrorCode, ToolName, ToolUiPayload},
};

use super::*;
#[tracing::instrument(skip(state, event_bus))]
pub async fn approve_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    use crate::app_state::core::EditProposalStatus;
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate_background(
            state,
            event_bus,
            Uuid::new_v4(),
            msg,
            MessageKind::SysInfo,
        )
        .await
    };
    let reg = state.proposals.write().await;
    let Some(proposal) = reg.get(&request_id).cloned() else {
        let msg = format!(
            "No staged edit proposal found for request_id {}",
            request_id
        );
        drop(reg);
        add_msg_imm(msg).await;
        return;
    };
    drop(reg);

    let is_semantic = proposal.is_semantic;
    // Idempotency checks (without holding lock)
    match proposal.status {
        EditProposalStatus::Pending => {
            let msg = format!("Applying patch for request_id {}", request_id);
            add_msg_imm(msg).await;
        }
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
        EditProposalStatus::Stale(_) => {
            tracing::debug!("Edit proposal marked stale, attempting edit again");
        }
    }

    // Apply edits via IoManagerHandle
    let file_paths = proposal.files.clone();
    if proposal.is_semantic {
        apply_semantic_edit(state, event_bus, request_id, proposal, file_paths).await;
    } else {
        apply_ns_edit(state, event_bus, request_id, proposal, file_paths).await;
    }
}

#[tracing::instrument(skip(state, event_bus))]
async fn apply_ns_edit(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    request_id: Uuid,
    mut proposal: crate::app_state::core::EditProposal,
    file_paths: Vec<PathBuf>,
) {
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate_background(
            state,
            event_bus,
            Uuid::new_v4(),
            msg,
            MessageKind::SysInfo,
        )
        .await;
    };

    match state
        .io_handle
        .write_batch_ns(proposal.edits_ns.clone())
        .await
    {
        Ok(results) => {
            let applied = results
                .iter()
                .inspect(|r| {
                    if r.is_err() {
                        tracing::error!("Error while applying patch: {r:?}");
                    }
                })
                .filter(|r| r.is_ok())
                .count();
            let file_count = file_paths.len();
            let results_json: Vec<serde_json::Value> = results
                .into_iter()
                .zip(file_paths.into_iter())
                .map(|(res, path)| match res {
                    Ok(write_res) => serde_json::json!({
                        "file_path": path.display().to_string(),
                        "new_file_hash": format!("{:?}", write_res.new_file_hash ),
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
            let tool_name = if proposal.is_semantic {
                ToolName::ApplyCodeEdit
            } else {
                ToolName::NsPatch
            };
            let mut reg = state.proposals.write().await;
            reg.insert(request_id, proposal);
            drop(reg);
            let ui_payload = ToolUiPayload::new(
                tool_name,
                call_id_val.clone(),
                format!("Applied {} edits across {} files", applied, file_count),
            )
            .with_request_id(request_id)
            .with_field("status", "applied")
            .with_field("ok", (applied > 0).to_string())
            .with_field("applied", applied.to_string())
            .with_field("files", file_count.to_string());
            let ui_payload_for_chat = ui_payload.clone();
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
                    content: content.clone(),
                    ui_payload: Some(ui_payload),
                }));
            chat::update_tool_message_by_call_id(
                state,
                event_bus,
                &call_id_val,
                Some(content.clone()),
                Some(ui_payload_for_chat),
            )
            .await;

            let msg = format!("Applied edits for request_id {}", request_id);
            add_msg_imm(msg).await;

            // Persist proposals (best-effort)
            crate::app_state::handlers::proposals::save_proposals(state).await;

            // Surface a brief SysInfo so users see that a rescan has been scheduled

            let msg = "Scheduled rescan of workspace after applying edits".to_string();
            add_msg_imm(msg).await;
        }
        Err(e) => {
            proposal.status = EditProposalStatus::Failed(e.to_string());
            let parent_id_val = proposal.parent_id;
            let call_id_val = proposal.call_id.clone();
            let tool_name = if proposal.is_semantic {
                ToolName::ApplyCodeEdit
            } else {
                ToolName::NsPatch
            };
            let mut reg = state.proposals.write().await;
            reg.insert(request_id, proposal);
            drop(reg);
            let err_str = format!("Failed to apply edits: {}", e);
            let err = ToolError::new(tool_name, ToolErrorCode::Io, err_str.clone());
            let ui_payload = ToolUiPayload::from_error(call_id_val.clone(), &err)
                .with_request_id(request_id)
                .with_field("status", "failed");
            let ui_payload_for_chat = ui_payload.clone();
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
                    error: err.to_wire_string(),
                    ui_payload: Some(ui_payload),
                }));
            chat::update_tool_message_by_call_id(
                state,
                event_bus,
                &call_id_val,
                Some(err.to_wire_string()),
                Some(ui_payload_for_chat),
            )
            .await;

            let msg = format!("Failed to apply edits for request_id {}: {}", request_id, e);
            add_msg_imm(msg).await;
            crate::app_state::handlers::proposals::save_proposals(state).await;
        }
    }
}

#[tracing::instrument(skip(state, event_bus))]
async fn apply_semantic_edit(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    request_id: Uuid,
    mut proposal: crate::app_state::core::EditProposal,
    file_paths: Vec<PathBuf>,
) {
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate_background(
            state,
            event_bus,
            Uuid::new_v4(),
            msg,
            MessageKind::SysInfo,
        )
        .await;
    };

    match state
        .io_handle
        .write_snippets_batch(proposal.edits.clone())
        .await
    {
        Ok(results) => {
            let applied = results.iter().filter(|r| r.is_ok()).count();
            let file_count = file_paths.len();
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
            let tool_name = if proposal.is_semantic {
                ToolName::ApplyCodeEdit
            } else {
                ToolName::NsPatch
            };
            let mut reg = state.proposals.write().await;
            reg.insert(request_id, proposal);
            drop(reg);
            let ui_payload = ToolUiPayload::new(
                tool_name,
                call_id_val.clone(),
                format!("Applied {} edits across {} files", applied, file_count),
            )
            .with_request_id(request_id)
            .with_field("status", "applied")
            .with_field("ok", (applied > 0).to_string())
            .with_field("applied", applied.to_string())
            .with_field("files", file_count.to_string());
            let ui_payload_for_chat = ui_payload.clone();
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
                    content: content.clone(),
                    ui_payload: Some(ui_payload),
                }));
            chat::update_tool_message_by_call_id(
                state,
                event_bus,
                &call_id_val,
                Some(content.clone()),
                Some(ui_payload_for_chat),
            )
            .await;

            let msg = format!("Applied edits for request_id {}", request_id);
            add_msg_imm(msg).await;

            // Persist proposals (best-effort)
            crate::app_state::handlers::proposals::save_proposals(state).await;

            // Post-apply: trigger a rescan to refresh indexes after semantic edits only,
            // e.g. not after `NsPatch`
            rescan_for_changes(state, event_bus, request_id);
            // Surface a brief SysInfo so users see that a rescan has been scheduled

            let msg = "Scheduled rescan of workspace after applying edits".to_string();
            add_msg_imm(msg).await;
        }
        Err(e) => {
            proposal.status = EditProposalStatus::Failed(e.to_string());
            let parent_id_val = proposal.parent_id;
            let call_id_val = proposal.call_id.clone();
            let tool_name = if proposal.is_semantic {
                ToolName::ApplyCodeEdit
            } else {
                ToolName::NsPatch
            };
            let mut reg = state.proposals.write().await;
            reg.insert(request_id, proposal);
            drop(reg);
            let err_str = format!("Failed to apply edits: {}", e);
            let err = ToolError::new(tool_name, ToolErrorCode::Io, err_str.clone());
            let ui_payload = ToolUiPayload::from_error(call_id_val.clone(), &err)
                .with_request_id(request_id)
                .with_field("status", "failed");
            let ui_payload_for_chat = ui_payload.clone();
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
                    error: err.to_wire_string(),
                    ui_payload: Some(ui_payload),
                }));
            chat::update_tool_message_by_call_id(
                state,
                event_bus,
                &call_id_val,
                Some(err.to_wire_string()),
                Some(ui_payload_for_chat),
            )
            .await;

            let msg = format!("Failed to apply edits for request_id {}: {}", request_id, e);
            add_msg_imm(msg).await;
            crate::app_state::handlers::proposals::save_proposals(state).await;
        }
    }
}

fn rescan_for_changes(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    let (scan_tx, scan_rx) = tokio::sync::oneshot::channel();
    tokio::spawn({
        let state = Arc::clone(state);
        let event_bus = Arc::clone(event_bus);
        async move {
            crate::app_state::handlers::db::scan_for_change(&state, &event_bus, scan_tx).await;
            let add_chat_message = |msg: String| {
                chat::add_msg_immediate_background(
                    &state,
                    &event_bus,
                    Uuid::new_v4(),
                    msg.to_string(),
                    MessageKind::SysInfo,
                )
            };
            match scan_rx.await {
                Ok(Some(files_changed)) => {
                    let changed_string = files_changed.iter().map(|f| f.to_string_lossy()).fold(
                        String::new(),
                        |mut acc, s| {
                            acc.push_str(&s);
                            acc.push('\n');
                            acc
                        },
                    );
                    let msg = format!("Files noted as having changed:\n{:?}", changed_string);
                    tracing::info!(target: "edit-proposals", msg);
                    add_chat_message(msg).await;
                }
                Ok(None) => {
                    let msg = "No changed files detected".to_string();
                    tracing::info!(target: "edit-proposals", msg);
                    add_chat_message(msg).await;
                }
                Err(e) => {
                    let msg = format!(
                        "Error scanning workspace for changes in request id {}\nError: {}",
                        request_id, e
                    );
                    tracing::error!(target: "edit-proposals", msg);
                    add_chat_message(msg).await;
                }
            }
        }
    });
}

pub async fn deny_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    use crate::app_state::core::EditProposalStatus;
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate_background(
            state,
            event_bus,
            Uuid::new_v4(),
            msg,
            MessageKind::SysInfo,
        )
        .await
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
        | EditProposalStatus::Failed(_)
        | EditProposalStatus::Stale(_) => {
            proposal.status = EditProposalStatus::Denied;
            let parent_id_val = proposal.parent_id;
            let call_id_val = proposal.call_id.clone();
            let tool_name = if proposal.is_semantic {
                ToolName::ApplyCodeEdit
            } else {
                ToolName::NsPatch
            };
            let mut reg = state.proposals.write().await;
            reg.insert(request_id, proposal);
            drop(reg);

            // Bridge: mark tool call failed with denial
            let err_msg = "Edit proposal denied by user".to_string();
            let err = ToolError::new(tool_name, ToolErrorCode::Internal, err_msg.clone());
            let ui_payload = ToolUiPayload::from_error(call_id_val.clone(), &err)
                .with_request_id(request_id)
                .with_field("status", "denied");
            let ui_payload_for_chat = ui_payload.clone();
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
                    error: err.to_wire_string(),
                    ui_payload: Some(ui_payload),
                }));
            chat::update_tool_message_by_call_id(
                state,
                event_bus,
                &call_id_val,
                Some(err.to_wire_string()),
                Some(ui_payload_for_chat),
            )
            .await;

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

/// Approve all pending edit proposals in a single pass.
///
/// Overlap handling: proposals are sorted newest-first, and only the most
/// recent proposal is applied when edits overlap in the same file. Older
/// overlapping proposals are marked stale with a note explaining why.
#[tracing::instrument(skip(state, event_bus))]
pub async fn approve_pending_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>) {
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate_background(
            state,
            event_bus,
            Uuid::new_v4(),
            msg,
            MessageKind::SysInfo,
        )
        .await
    };

    let mut pending: Vec<crate::app_state::core::EditProposal> = {
        let reg = state.proposals.read().await;
        reg.values()
            .filter(|p| matches!(p.status, EditProposalStatus::Pending))
            .cloned()
            .collect()
    };

    if pending.is_empty() {
        add_msg_imm("No pending edit proposals to approve".to_string()).await;
        return;
    }

    pending.sort_by(|a, b| {
        b.proposed_at_ms
            .cmp(&a.proposed_at_ms)
            .then(b.request_id.cmp(&a.request_id))
    });

    let mut occupied: HashMap<PathBuf, Vec<(usize, usize)>> = HashMap::new();
    let mut to_apply: Vec<Uuid> = Vec::new();
    let mut to_stale: Vec<Uuid> = Vec::new();

    for proposal in pending.iter() {
        let ranges = proposal_ranges(proposal);
        if overlaps_existing(&occupied, &ranges) {
            to_stale.push(proposal.request_id);
        } else {
            mark_occupied(&mut occupied, &ranges);
            to_apply.push(proposal.request_id);
        }
    }

    if !to_stale.is_empty() {
        let mut reg = state.proposals.write().await;
        for request_id in &to_stale {
            if let Some(p) = reg.get_mut(request_id) {
                p.status = EditProposalStatus::Stale(
                    "Overlaps with newer edit proposal".to_string(),
                );
            }
        }
        drop(reg);
        for request_id in &to_stale {
            add_msg_imm(format!(
                "Skipped edits for request_id {} (overlaps with newer proposal)",
                request_id
            ))
            .await;
        }
    }

    for request_id in to_apply {
        approve_edits(state, event_bus, request_id).await;
    }

    crate::app_state::handlers::proposals::save_proposals(state).await;
}

/// Deny every currently pending edit proposal.
///
/// This is a bulk convenience path that mirrors `deny_edits` behavior per
/// proposal and reports when there is nothing to deny.
#[tracing::instrument(skip(state, event_bus))]
pub async fn deny_pending_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>) {
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate_background(
            state,
            event_bus,
            Uuid::new_v4(),
            msg,
            MessageKind::SysInfo,
        )
        .await
    };

    let pending_ids: Vec<Uuid> = {
        let reg = state.proposals.read().await;
        reg.values()
            .filter(|p| matches!(p.status, EditProposalStatus::Pending))
            .map(|p| p.request_id)
            .collect()
    };

    if pending_ids.is_empty() {
        add_msg_imm("No pending edit proposals to deny".to_string()).await;
        return;
    }

    for request_id in pending_ids {
        deny_edits(state, event_bus, request_id).await;
    }
}

/// Extract file ranges touched by a proposal for overlap detection.
///
/// For semantic edits, ranges are derived from explicit byte spans.
/// For non-semantic edits or missing spans, the entire file is treated
/// as touched to ensure we do not apply overlapping changes out of order.
fn proposal_ranges(
    proposal: &crate::app_state::core::EditProposal,
) -> Vec<(PathBuf, usize, usize)> {
    let mut ranges = Vec::new();

    for edit in &proposal.edits {
        let (start, end) = normalize_range(edit.start_byte, edit.end_byte);
        ranges.push((edit.file_path.clone(), start, end));
    }

    for edit in &proposal.edits_ns {
        ranges.push((edit.file_path.clone(), 0, usize::MAX));
    }

    if ranges.is_empty() {
        for path in &proposal.files {
            ranges.push((path.clone(), 0, usize::MAX));
        }
    }

    ranges
}

/// Normalize a byte range to a non-empty, ordered interval.
///
/// This prevents zero-width ranges from incorrectly skipping overlap checks.
fn normalize_range(start: usize, end: usize) -> (usize, usize) {
    let (min, max) = if start <= end { (start, end) } else { (end, start) };
    if min == max {
        (min, min.saturating_add(1))
    } else {
        (min, max)
    }
}

/// Check if any proposed ranges overlap previously accepted ranges.
fn overlaps_existing(
    occupied: &HashMap<PathBuf, Vec<(usize, usize)>>,
    ranges: &[(PathBuf, usize, usize)],
) -> bool {
    for (path, start, end) in ranges {
        if let Some(existing) = occupied.get(path) {
            for (ex_start, ex_end) in existing {
                if start < ex_end && ex_start < end {
                    return true;
                }
            }
        }
    }
    false
}

/// Record accepted ranges for a proposal so older ones can be skipped when overlapping.
fn mark_occupied(
    occupied: &mut HashMap<PathBuf, Vec<(usize, usize)>>,
    ranges: &[(PathBuf, usize, usize)],
) {
    for (path, start, end) in ranges {
        occupied
            .entry(path.clone())
            .or_default()
            .push((*start, *end));
    }
}

pub async fn approve_creations(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    use crate::app_state::core::EditProposalStatus;
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate_background(
            state,
            event_bus,
            Uuid::new_v4(),
            msg,
            MessageKind::SysInfo,
        )
        .await
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
        EditProposalStatus::Pending => {
            tracing::debug!("Creations has pending proposal for request_id {request_id}");
        }
        EditProposalStatus::Applied => {
            let msg = format!("Creations already applied for request_id {request_id}");
            add_msg_imm(msg).await;
            return;
        }
        EditProposalStatus::Denied => {
            let msg = format!("Creations already denied for request_id {request_id}");
            add_msg_imm(msg).await;
            return;
        }
        EditProposalStatus::Approved => {
            tracing::debug!("Create-file proposal approved, applying for request_id {request_id}");
        }
        EditProposalStatus::Failed(e) => {
            tracing::debug!(
                "Create-file proposal for request_id {request_id} previously failed with error: {e};\nRetrying apply..."
            );
        }
        EditProposalStatus::Stale(e) => {
            tracing::debug!(
                "Create-file proposal for request_id {request_id} marked stale: {e}; retrying apply..."
            );
        }
    }

    // Apply creations via IoManagerHandle
    let file_paths = proposal.files.clone();
    let file_count = file_paths.len();
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

    let ui_payload = ToolUiPayload::new(
        ToolName::CreateFile,
        call_id_val.clone(),
        format!("Applied {} creations across {} files", applied, file_count),
    )
    .with_request_id(request_id)
    .with_field("status", "applied")
    .with_field("ok", (applied > 0).to_string())
    .with_field("applied", applied.to_string())
    .with_field("files", file_count.to_string());
    let ui_payload_for_chat = ui_payload.clone();
    let _ = event_bus
        .realtime_tx
        .send(AppEvent::System(SystemEvent::ToolCallCompleted {
            request_id,
            parent_id: parent_id_val,
            call_id: call_id_val.clone(),
            content: content.clone(),
            ui_payload: Some(ui_payload),
        }));
    chat::update_tool_message_by_call_id(
        state,
        event_bus,
        &call_id_val,
        Some(content.clone()),
        Some(ui_payload_for_chat),
    )
    .await;

    chat::add_msg_immediate_background(
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
            let _ = scan_rx
                .await
                .inspect_err(|e| tracing::error!(scan_error = ?e));
        }
    });
    chat::add_msg_immediate_background(
        state,
        event_bus,
        Uuid::new_v4(),
        "Scheduled rescan of workspace after creating files".to_string(),
        MessageKind::SysInfo,
    )
    .await;
}

pub async fn deny_creations(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    use crate::app_state::core::EditProposalStatus;
    let add_msg_imm = async move |msg: String| {
        chat::add_msg_immediate_background(
            state,
            event_bus,
            Uuid::new_v4(),
            msg,
            MessageKind::SysInfo,
        )
        .await
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
        EditProposalStatus::Pending
        | EditProposalStatus::Approved
        | EditProposalStatus::Failed(_)
        | EditProposalStatus::Stale(_) => {
            proposal.status = EditProposalStatus::Denied;
            let parent_id_val = proposal.parent_id;
            let call_id_val = proposal.call_id.clone();
            let mut regw = state.create_proposals.write().await;
            regw.insert(request_id, proposal);
            drop(regw);

            // Bridge: mark tool call failed with denial
            let err_msg = "Create-file proposal denied by user".to_string();
            let err = ToolError::new(
                ToolName::CreateFile,
                ToolErrorCode::Internal,
                err_msg.clone(),
            );
            let ui_payload = ToolUiPayload::from_error(call_id_val.clone(), &err)
                .with_request_id(request_id)
                .with_field("status", "denied");
            let ui_payload_for_chat = ui_payload.clone();
            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    parent_id: parent_id_val,
                    call_id: call_id_val.clone(),
                    error: err.to_wire_string(),
                    ui_payload: Some(ui_payload),
                }));
            chat::update_tool_message_by_call_id(
                state,
                event_bus,
                &call_id_val,
                Some(err.to_wire_string()),
                Some(ui_payload_for_chat),
            )
            .await;

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
