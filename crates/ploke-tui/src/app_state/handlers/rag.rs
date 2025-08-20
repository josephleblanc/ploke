use ploke_core::{PROJECT_NAMESPACE_UUID, TrackingHash, WriteSnippetData};
use ploke_rag::AssembledContext;
use ploke_rag::RagService;
use ploke_rag::RetrievalStrategy;
use ploke_rag::RrfConfig;
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use uuid::Uuid;
use sha2::{Digest, Sha256};
use similar::TextDiff;
use std::collections::{BTreeMap, HashMap};

use crate::AppEvent;
use crate::EventBus;
use crate::chat_history::Message;
use crate::chat_history::MessageKind;
use crate::error::ErrorExt;
use crate::llm;
use crate::llm::ToolEvent;
use crate::system::SystemEvent;

use crate::AppState;
use crate::RagEvent;

use super::embedding::wait_on_oneshot;

static PROMPT_HEADER: &str = r#"
<-- SYSTEM PROMPT -->
You are a highly skilled software engineer, specializing in the Rust programming language.

You will be asked to provide some assistance in collaborating with the user.
"#;

static PROMPT_CODE: &str = r#"
Next, you will be provided with some of the user's code, that has been retrieved
to provide helpful context for you to answer their questions. This context will
be provided within code tags like these:

<code="path/to/file.rs" #132:486>Code goes here</code>

Where the "path/to/file.rs" is the absolute path to the file and the #132:486
are the line numbers, inclusive.

What follows is the provided code snippets for you to use as reference, and will
be shown in a header (like # Header) and with subheaders (like ## subheader).
Follow the code section will be the User's query, delineated by a header.

After the user query, there may be a response from another collaborator marked
with a header (like # Assistant or # Collaborator). These headers may alternate
and contain subheaders with the whole text of their messages so far, summaries
of the conversation, or other contextual information about the code base.

# Code

"#;
static PROMPT_USER: &str = r#"
# USER

"#;

#[tracing::instrument(skip(state, event_bus, arguments), fields(%request_id, %parent_id, call_id = %call_id, tool = %name))]
pub async fn handle_tool_call_requested(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    request_id: Uuid,
    parent_id: Uuid,
    vendor: llm::ToolVendor,
    name: String,
    arguments: serde_json::Value,
    call_id: String,
) {
    tracing::info!(
        "handle_tool_call_requested: vendor={:?}, name={}",
        vendor,
        name
    );
    tracing::warn!(
        "DEPRECATED PATH: SystemEvent::ToolCallRequested execution path is deprecated; will be refactored into dedicated tool events. Kept for compatibility."
    );
    let tool_call_failed = |error| {
        AppEvent::System(SystemEvent::ToolCallFailed {
            request_id,
            parent_id,
            call_id: call_id.clone(),
            error,
        })
    };

    // Handle atomic code edit application via ploke-io (M1: stage proposal, do not apply immediately)
    if name == "apply_code_edit" {
        // Idempotency: guard duplicate requests
        {
            let reg = state.proposals.read().await;
            if reg.contains_key(&request_id) {
                let msg = format!(
                    "Duplicate apply_code_edit request ignored for request_id {}",
                    request_id
                );
                // Bridge + typed failure for idempotent duplicate
                let _ = event_bus.realtime_tx.send(AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id,
                    parent_id,
                    call_id: call_id.clone(),
                    error: "Duplicate request_id".to_string(),
                }));
                event_bus.send(AppEvent::LlmTool(ToolEvent::Failed {
                    request_id,
                    parent_id,
                    call_id: call_id.clone(),
                    error: "Duplicate request_id".to_string(),
                }));
                super::chat::add_msg_immediate(
                    state,
                    event_bus,
                    Uuid::new_v4(),
                    msg,
                    crate::chat_history::MessageKind::SysInfo,
                )
                .await;
                return;
            }
        }

        // Parse optional confidence (kept for future gating)
        let _confidence = arguments
            .get("confidence")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32);

        // Parse namespace or default to PROJECT_NAMESPACE_UUID
        let namespace = arguments
            .get("namespace")
            .and_then(|v| v.as_str())
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .unwrap_or(PROJECT_NAMESPACE_UUID);

        // Parse edits array
        let Some(edits_arr) = arguments.get("edits").and_then(|v| v.as_array()) else {
            let _ = event_bus
                .realtime_tx
                .send(tool_call_failed("Missing or invalid 'edits' array".to_string()));
            return;
        };

        if edits_arr.is_empty() {
            let _ = event_bus
                .realtime_tx
                .send(tool_call_failed("No edits provided".to_string()));
            event_bus.send(AppEvent::LlmTool(ToolEvent::Failed {
                request_id,
                parent_id,
                call_id: call_id.clone(),
                error: "No edits provided".to_string(),
            }));
            super::chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                "apply_code_edit: No edits provided".to_string(),
                crate::chat_history::MessageKind::SysInfo,
            )
            .await;
            return;
        }

        let mut edits: Vec<WriteSnippetData> = Vec::with_capacity(edits_arr.len());
        let mut files_set: std::collections::BTreeSet<std::path::PathBuf> = std::collections::BTreeSet::new();
        let mut interval_map: std::collections::BTreeMap<std::path::PathBuf, Vec<(usize, usize)>> =
            std::collections::BTreeMap::new();

        for e in edits_arr {
            let Some(file_path) = e.get("file_path").and_then(|v| v.as_str()) else {
                let _ = event_bus
                    .realtime_tx
                .send(tool_call_failed("Edit missing 'file_path'".to_string()));
                return;
            };
            let Some(hash_str) = e.get("expected_file_hash").and_then(|v| v.as_str()) else {
                let _ = event_bus
                    .realtime_tx
                .send(tool_call_failed("Edit missing 'expected_file_hash'".to_string()));
                return;
            };
            let Some(start_byte) = e.get("start_byte").and_then(|v| v.as_u64()) else {
                let _ = event_bus
                    .realtime_tx
                .send(tool_call_failed("Edit missing 'start_byte'".to_string()));
                return;
            };
            let Some(end_byte) = e.get("end_byte").and_then(|v| v.as_u64()) else {
                let _ = event_bus
                    .realtime_tx
                    .send(tool_call_failed("Edit missing 'end_byte'".to_string()));
                return;
            };
            if end_byte < start_byte {
                let _ = event_bus
                    .realtime_tx
                    .send(tool_call_failed(
                        "Invalid edit range: end_byte < start_byte".to_string(),
                    ));
                event_bus.send(AppEvent::LlmTool(ToolEvent::Failed {
                    request_id,
                    parent_id,
                    call_id: call_id.clone(),
                    error: "Invalid edit range: end_byte < start_byte".to_string(),
                }));
                super::chat::add_msg_immediate(
                    state,
                    event_bus,
                    Uuid::new_v4(),
                    format!(
                        "apply_code_edit: Invalid range end({}) < start({}) for file {}",
                        end_byte, start_byte, file_path
                    ),
                    crate::chat_history::MessageKind::SysInfo,
                )
                .await;
                return;
            }
            // Collect for overlap validation
            interval_map
                .entry(std::path::PathBuf::from(file_path))
                .or_default()
                .push((start_byte as usize, end_byte as usize));
            let Some(replacement) = e.get("replacement").and_then(|v| v.as_str()) else {
                let _ = event_bus
                    .realtime_tx
                    .send(tool_call_failed("Edit missing 'replacement'".to_string()));
                return;
            };

            let Ok(hash_uuid) = uuid::Uuid::parse_str(hash_str) else {
                let _ = event_bus
                    .realtime_tx
                .send(tool_call_failed(format!("Invalid expected_file_hash UUID: {}", hash_str)));
                return;
            };

            let path_buf = std::path::PathBuf::from(file_path);
            let ws = WriteSnippetData {
                id: uuid::Uuid::new_v4(),
                name: "edit".to_string(),
                file_path: path_buf.clone(),
                expected_file_hash: TrackingHash(hash_uuid),
                start_byte: start_byte as usize,
                end_byte: end_byte as usize,
                replacement: replacement.to_string(),
                namespace,
            };
            files_set.insert(path_buf);
            edits.push(ws);
        }

        // Validate non-overlapping ranges per file
        for (path, ranges) in interval_map.iter_mut() {
            ranges.sort_by_key(|(s, _)| *s);
            let mut prev_end = 0usize;
            for (i, (s, e)) in ranges.iter().enumerate() {
                if i > 0 && *s < prev_end {
                    let msg = format!("Overlapping edit ranges for {}", path.display());
                    let _ = event_bus.realtime_tx.send(tool_call_failed(msg.clone()));
                    event_bus.send(AppEvent::LlmTool(ToolEvent::Failed {
                        request_id,
                        parent_id,
                        call_id: call_id.clone(),
                        error: msg.clone(),
                    }));
                    super::chat::add_msg_immediate(
                        state,
                        event_bus,
                        Uuid::new_v4(),
                        msg,
                        crate::chat_history::MessageKind::SysInfo,
                    )
                    .await;
                    return;
                }
                prev_end = *e;
            }
        }

        // Compute a simple args hash for auditing
        let mut hasher = Sha256::new();
        hasher.update(arguments.to_string().as_bytes());
        let args_hash = format!("{:x}", hasher.finalize());

        // Build a lightweight preview (code-block stubs for now; unified diff optional in later step)
        let mut per_file: Vec<crate::app_state::core::BeforeAfter> = Vec::new();
        let mut unified_diff = String::new();
        // Normalize display paths relative to current crate focus when available
        let crate_root = { state.system.read().await.crate_focus.clone() };
        let editing_cfg = { state.config.read().await.editing.clone() };
        for path in files_set.iter() {
            let before = match tokio::fs::read_to_string(path).await {
                Ok(s) => s,
                Err(_) => "<unreadable or binary file>".to_string(),
            };

            // Apply all edits for this file in-memory (descending by start to keep indices stable)
            let mut bytes = before.clone().into_bytes();
            let mut file_edits: Vec<&WriteSnippetData> = edits.iter().filter(|e| &e.file_path == path).collect();
            file_edits.sort_by_key(|e| e.start_byte);
            file_edits.reverse();
            for e in file_edits {
                let start = e.start_byte.min(bytes.len());
                let end = e.end_byte.min(bytes.len());
                if start > end {
                    continue;
                }
                let mut new_bytes = Vec::with_capacity(bytes.len() + e.replacement.len());
                new_bytes.extend_from_slice(&bytes[..start]);
                new_bytes.extend_from_slice(e.replacement.as_bytes());
                new_bytes.extend_from_slice(&bytes[end..]);
                bytes = new_bytes;
            }
            let after = String::from_utf8_lossy(&bytes).to_string();

            let display_path = if let Some(root) = crate_root.as_ref() {
                path.strip_prefix(root).unwrap_or(path.as_path()).to_path_buf()
            } else {
                path.clone()
            };
            per_file.push(crate::app_state::core::BeforeAfter {
                file_path: display_path.clone(),
                before: before.clone(),
                after: after.clone(),
            });
            if matches!(editing_cfg.preview_mode, crate::app_state::core::PreviewMode::Diff) {
                let header_a = format!("a/{}", display_path.display());
                let header_b = format!("b/{}", display_path.display());
                let diff = TextDiff::from_lines(&before, &after)
                    .unified_diff()
                    .header(&header_a, &header_b)
                    .to_string();
                unified_diff.push_str(&diff);
                if !unified_diff.ends_with('\n') {
                    unified_diff.push('\n');
                }
            }
        }

        let files: Vec<std::path::PathBuf> = files_set.into_iter().collect();
        let display_files: Vec<String> = files
            .iter()
            .map(|p| {
                if let Some(root) = crate_root.as_ref() {
                    p.strip_prefix(root)
                        .map(|rp| rp.display().to_string())
                        .unwrap_or_else(|_| p.display().to_string())
                } else {
                    p.display().to_string()
                }
            })
            .collect();

        // Build preview snippet for SysInfo (truncated per config)
        let preview_label = if matches!(editing_cfg.preview_mode, crate::app_state::core::PreviewMode::Diff) {
            "diff"
        } else {
            "codeblock"
        };

        let truncate = |s: &str| -> String {
            let max = editing_cfg.max_preview_lines;
            let mut out = String::new();
            for (i, line) in s.lines().enumerate() {
                if i >= max {
                    out.push_str("\n... [truncated]\n");
                    break;
                }
                out.push_str(line);
                out.push('\n');
            }
            out
        };

        let preview_snippet = if matches!(editing_cfg.preview_mode, crate::app_state::core::PreviewMode::Diff) {
            truncate(&unified_diff)
        } else {
            let mut buf = String::new();
            for ba in &per_file {
                buf.push_str(&format!(
                    "--- {}\nBefore:\n```\n{}\n```\nAfter:\n```\n{}\n```\n",
                    ba.file_path.display(),
                    truncate(&ba.before),
                    truncate(&ba.after)
                ));
            }
            buf
        };

        // Stash proposal into in-memory registry
        {
            use crate::app_state::core::{DiffPreview, EditProposal, EditProposalStatus};
            let mut reg = state.proposals.write().await;
            reg.insert(
                request_id,
                EditProposal {
                    request_id,
                    parent_id,
                    call_id: call_id.clone(),
                    proposed_at_ms: chrono::Utc::now().timestamp_millis(),
                    edits,
                    files: files.clone(),
                    args_hash,
                    preview: if matches!(editing_cfg.preview_mode, crate::app_state::core::PreviewMode::Diff) {
                        crate::app_state::core::DiffPreview::UnifiedDiff { text: unified_diff.clone() }
                    } else {
                        crate::app_state::core::DiffPreview::CodeBlocks { per_file: per_file.clone() }
                    },
                    status: EditProposalStatus::Pending,
                },
            );
        }

        // Emit a concise SysInfo message with how to approve/deny
        let summary = format!(
            "Staged code edits (request_id: {}, call_id: {}).\nFiles:\n  {}\n\nPreview (mode={}, first {} lines per section):\n{}\n\nApprove:  edit approve {}\nDeny:     edit deny {}{}",
            request_id,
            call_id,
            display_files.join("\n  "),
            preview_label,
            editing_cfg.max_preview_lines,
            preview_snippet,
            request_id,
            request_id,
            if editing_cfg.auto_confirm_edits { "\n\nAuto-approval enabled: applying now..." } else { "" }
        );
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            summary,
            crate::chat_history::MessageKind::SysInfo,
        )
        .await;

        if editing_cfg.auto_confirm_edits {
            let state2 = Arc::clone(state);
            let event_bus2 = Arc::clone(event_bus);
            tokio::spawn(async move {
                approve_edits(&state2, &event_bus2, request_id).await;
            });
        }

        // Do not send Completed/Failed now; wait for user approval (unless auto-approval enabled)
        return;
    }

    if name != "request_code_context" {
        tracing::warn!("Unsupported tool call: {}", name);
        let err = format!("Unsupported tool: {}", name);
        let _ = event_bus
            .realtime_tx
            .send(tool_call_failed(err.clone()));
        event_bus.send(AppEvent::LlmTool(ToolEvent::Failed {
            request_id,
            parent_id,
            call_id: call_id.clone(),
            error: err,
        }));
        return;
    }

    // Parse arguments
    let token_budget = arguments
        .get("token_budget")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    if token_budget.is_none() || token_budget == Some(0) {
        let msg = "Invalid or missing token_budget".to_string();
        let _ = event_bus
            .realtime_tx
            .send(tool_call_failed(msg.clone()));
        event_bus.send(AppEvent::LlmTool(ToolEvent::Failed {
            request_id,
            parent_id,
            call_id: call_id.clone(),
            error: msg,
        }));
        return;
    }
    let token_budget = token_budget.unwrap();
    let hint = arguments
        .get("hint")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Determine query: prefer hint, otherwise last user message
    let query = if let Some(h) = hint.filter(|s| !s.trim().is_empty()) {
        h
    } else {
        let guard = state.chat.read().await;
        match guard.last_user_msg() {
            Ok(Some((_id, content))) => content,
            _ => String::new(),
        }
    };

    if query.trim().is_empty() {
        let msg = "No query available (no hint provided and no recent user message)".to_string();
        let _ = event_bus
            .realtime_tx
            .send(tool_call_failed(msg.clone()));
        event_bus.send(AppEvent::LlmTool(ToolEvent::Failed {
            request_id,
            parent_id,
            call_id: call_id.clone(),
            error: msg,
        }));
        return;
    }

    let top_k = calc_top_k_for_budget(token_budget);

    if let Some(rag) = &state.rag {
        match rag.hybrid_search(&query, top_k).await {
            Ok(results) => {
                let results_json: Vec<serde_json::Value> = results
                    .into_iter()
                    .map(|(id, score)| serde_json::json!({"id": id.to_string(), "score": score}))
                    .collect();

                let content = serde_json::json!({
                    "ok": true,
                    "query": query,
                    "top_k": top_k,
                    "results": results_json
                })
                .to_string();

                let _ =
                    event_bus
                        .realtime_tx
                        .send(AppEvent::System(SystemEvent::ToolCallCompleted {
                            request_id,
                            parent_id,
                            call_id: call_id.clone(),
                            content,
                        }));
            }
            Err(e) => {
                let msg = format!("RAG hybrid_search failed: {}", e);
                tracing::warn!("{}", msg);
                let _ = event_bus
                    .realtime_tx
                    .send(tool_call_failed(msg.clone()));
                event_bus.send(AppEvent::LlmTool(ToolEvent::Failed {
                    request_id,
                    parent_id,
                    call_id: call_id.clone(),
                    error: msg,
                }));
            }
        }
    } else {
        let msg = "RAG service unavailable".to_string();
        tracing::warn!("{}", msg);
        let _ = event_bus
            .realtime_tx
            .send(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id,
                parent_id,
                call_id: call_id.clone(),
                error: msg.clone(),
            }));
        event_bus.send(AppEvent::LlmTool(ToolEvent::Failed {
            request_id,
            parent_id,
            call_id,
            error: msg,
        }));
    }
}

pub async fn approve_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    use crate::app_state::core::EditProposalStatus;
    let mut reg = state.proposals.write().await;
    let Some(mut proposal) = reg.get(&request_id).cloned() else {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            format!("No staged edit proposal found for request_id {}", request_id),
            crate::chat_history::MessageKind::SysInfo,
        )
        .await;
        return;
    };

    // Idempotency checks
    match proposal.status {
        EditProposalStatus::Pending => { /* ok */ }
        EditProposalStatus::Applied => {
            super::chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Edits already applied for request_id {}", request_id),
                crate::chat_history::MessageKind::SysInfo,
            )
            .await;
            return;
        }
        EditProposalStatus::Denied => {
            super::chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Edits already denied for request_id {}", request_id),
                crate::chat_history::MessageKind::SysInfo,
            )
            .await;
            return;
        }
        EditProposalStatus::Approved => {
            // Treat as attempting to apply again
        }
        EditProposalStatus::Failed(_) => {
            // Allow re-apply attempt
        }
    }

    // Apply edits via IoManagerHandle
    let file_paths = proposal.files.clone();
    match state.io_handle.write_snippets_batch(proposal.edits.clone()).await {
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
            let parent_id_val = reg.get(&request_id).map(|p| p.parent_id).unwrap_or_default();
            let call_id_val = reg.get(&request_id).map(|p| p.call_id.clone()).unwrap_or_default();
            let _ = event_bus.realtime_tx.send(AppEvent::System(SystemEvent::ToolCallCompleted {
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

            super::chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Applied edits for request_id {}", request_id),
                crate::chat_history::MessageKind::SysInfo,
            )
            .await;
        }
        Err(e) => {
            proposal.status = EditProposalStatus::Failed(e.to_string());
            reg.insert(request_id, proposal);

            let parent_id_val = reg.get(&request_id).map(|p| p.parent_id).unwrap_or_default();
            let call_id_val = reg.get(&request_id).map(|p| p.call_id.clone()).unwrap_or_default();
            let err_str = format!("Failed to apply edits: {}", e);
            let _ = event_bus.realtime_tx.send(AppEvent::System(SystemEvent::ToolCallFailed {
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

            super::chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Failed to apply edits for request_id {}: {}", request_id, e),
                crate::chat_history::MessageKind::SysInfo,
            )
            .await;
        }
    }
}

pub async fn deny_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    use crate::app_state::core::EditProposalStatus;
    let mut reg = state.proposals.write().await;
    let Some(mut proposal) = reg.get(&request_id).cloned() else {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            format!("No staged edit proposal found for request_id {}", request_id),
            crate::chat_history::MessageKind::SysInfo,
        )
        .await;
        return;
    };

    match proposal.status {
        EditProposalStatus::Pending | EditProposalStatus::Approved | EditProposalStatus::Failed(_) => {
            proposal.status = EditProposalStatus::Denied;
            reg.insert(request_id, proposal);

            // Bridge: mark tool call failed with denial
            let parent_id_val = reg.get(&request_id).map(|p| p.parent_id).unwrap_or_default();
            let call_id_val = reg.get(&request_id).map(|p| p.call_id.clone()).unwrap_or_default();
            let err_msg = "Edit proposal denied by user".to_string();
            let _ = event_bus.realtime_tx.send(AppEvent::System(SystemEvent::ToolCallFailed {
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

            super::chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Denied edits for request_id {}", request_id),
                crate::chat_history::MessageKind::SysInfo,
            )
            .await;
        }
        EditProposalStatus::Denied => {
            super::chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Edits already denied for request_id {}", request_id),
                crate::chat_history::MessageKind::SysInfo,
            )
            .await;
        }
        EditProposalStatus::Applied => {
            super::chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Edits already applied for request_id {}", request_id),
                crate::chat_history::MessageKind::SysInfo,
            )
            .await;
        }
    }
}

fn calc_top_k_for_budget(token_budget: u32) -> usize {
    let top_k = (token_budget / 200) as usize;
    if top_k < 5 {
        5
    } else if top_k > 20 {
        20
    } else {
        top_k
    }
}

pub async fn process_with_rag(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    scan_rx: oneshot::Receiver<Option<Vec<PathBuf>>>,
    new_msg_id: Uuid,
    completion_rx: oneshot::Receiver<()>,
) {
    if let ControlFlow::Break(_) = wait_on_oneshot(new_msg_id, completion_rx).await {
        return;
    }
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };
    if let Some(rag) = &state.rag {
        let guard = state.chat.read().await;

        let (msg_id, user_msg) = {
            match guard.last_user_msg().inspect_err(|e| e.emit_error()) {
                Ok(maybe_msg) => match maybe_msg {
                    Some(msg) => msg,
                    None => {
                        tracing::warn!("Attempting to submit empty user message");
                        return;
                    }
                },
                Err(e) => {
                    e.emit_error();
                    return;
                }
            }
        };
        let messages: Vec<Message> = guard.clone_current_path_conv();
        let budget = &state.budget;
        // TODO: Add this to the program config
        let top_k = 15;
        let retrieval_strategy = RetrievalStrategy::Hybrid {
            rrf: RrfConfig::default(),
            mmr: None,
        };
        let rag_ctx = match rag
            .get_context(&user_msg, top_k, budget, retrieval_strategy)
            .await
        {
            Ok(res) => res,
            Err(e) => {
                e.emit_error();
                tracing::error!("Failed to return results from hybrid RAG");
                return;
            }
        };
        let augmented_prompt = construct_context_from_rag(rag_ctx, messages, msg_id);

        event_bus.send(AppEvent::Llm(augmented_prompt));
    }
}

fn construct_context_from_rag(
    ctx: AssembledContext,
    messages: Vec<Message>,
    parent_id: Uuid,
) -> llm::Event {
    tracing::info!(
        "constructing context (RAG) with {} parts and {} messages",
        ctx.parts.len(),
        messages.len()
    );

    let mut base: Vec<(MessageKind, String)> = Vec::from([
        (MessageKind::System, String::from(PROMPT_HEADER)),
        (MessageKind::System, String::from(PROMPT_CODE)),
    ]);

    // Add assembled context parts as system messages
    let text = ctx.parts.into_iter().map(|p| (MessageKind::System, p.text));
    base.extend(text);

    // Add conversation messages
    let msgs = messages
        .into_iter()
        .filter(|m| m.kind == MessageKind::User || m.kind == MessageKind::Assistant)
        .inspect(|m| tracing::debug!("m.content.is_empty() = {}", m.content.is_empty()))
        .map(|msg| (msg.kind, msg.content));
    base.extend(msgs);

    llm::Event::PromptConstructed {
        parent_id,
        prompt: base,
    }
}

pub async fn bm25_rebuild(state: &Arc<AppState>, event_bus: &Arc<EventBus>) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

    if let Some(rag) = &state.rag {
        match rag.bm25_rebuild().await {
            Ok(()) => add_msg("BM25 rebuild requested").await,
            Err(e) => {
                let msg = format!("BM25 rebuild failed: {}", e);
                add_msg(&msg).await;
            }
        }
    } else {
        add_msg("RAG service unavailable; cannot rebuild BM25").await;
    }
}

pub async fn bm25_search(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    query: String,
    top_k: usize,
) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

    if let Some(rag) = &state.rag {
        match rag.search_bm25(&query, top_k).await {
            Ok(results) => {
                let lines: Vec<String> = results
                    .into_iter()
                    .map(|(id, score)| format!("{}: {:.3}", id, score))
                    .collect();
                let content = if lines.is_empty() {
                    format!("BM25 results (top {}): <no hits>", top_k)
                } else {
                    format!("BM25 results (top {}):\n{}", top_k, lines.join("\n"))
                };
                add_msg(&content).await;
            }
            Err(e) => {
                let msg = format!("BM25 search failed: {}", e);
                add_msg(&msg).await;
            }
        }
    } else {
        add_msg("RAG service unavailable; cannot run BM25 search").await;
    }
}

pub async fn hybrid_search(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    query: String,
    top_k: usize,
) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

    if let Some(rag) = &state.rag {
        match rag.hybrid_search(&query, top_k).await {
            Ok(results) => {
                let lines: Vec<String> = results
                    .into_iter()
                    .map(|(id, score)| format!("{}: {:.3}", id, score))
                    .collect();
                let content = if lines.is_empty() {
                    format!("Hybrid results (top {}): <no hits>", top_k)
                } else {
                    format!("Hybrid results (top {}):\n{}", top_k, lines.join("\n"))
                };
                add_msg(&content).await;
            }
            Err(e) => {
                let msg = format!("Hybrid search failed: {}", e);
                add_msg(&msg).await;
            }
        }
    } else {
        add_msg("RAG service unavailable; cannot run hybrid search").await;
    }
}

pub async fn bm25_status(state: &Arc<AppState>, event_bus: &Arc<EventBus>) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

    if let Some(rag) = &state.rag {
        match rag.bm25_status().await {
            Ok(status) => {
                let msg = format!("BM25 status: {:?}", status);
                add_msg(&msg).await;
            }
            Err(e) => {
                let msg = format!("BM25 status error: {}", e);
                add_msg(&msg).await;
            }
        }
    } else {
        add_msg("RAG service unavailable; cannot query BM25 status").await;
    }
}

pub async fn bm25_save(state: &Arc<AppState>, event_bus: &Arc<EventBus>, path: PathBuf) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

    if let Some(rag) = &state.rag {
        match rag.bm25_save(path.clone()).await {
            Ok(()) => {
                let msg = format!("BM25 index saved to {}", path.display());
                add_msg(&msg).await;
            }
            Err(e) => {
                let msg = format!("BM25 save failed: {}", e);
                add_msg(&msg).await;
            }
        }
    } else {
        add_msg("RAG service unavailable; cannot save BM25 index").await;
    }
}

pub async fn bm25_load(state: &Arc<AppState>, event_bus: &Arc<EventBus>, path: PathBuf) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

    if let Some(rag) = &state.rag {
        match rag.bm25_load(path.clone()).await {
            Ok(()) => {
                let msg = format!("BM25 index load requested from {}", path.display());
                add_msg(&msg).await;
            }
            Err(e) => {
                let msg = format!("BM25 load failed: {}", e);
                add_msg(&msg).await;
            }
        }
    } else {
        add_msg("RAG service unavailable; cannot load BM25 index").await;
    }
}

pub async fn sparse_search(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    req_id: Uuid,
    query: String,
    top_k: usize,
    strict: bool,
) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

    if let Some(rag) = &state.rag {
        let result = if strict {
            rag.search_bm25_strict(&query, top_k).await
        } else {
            rag.search_bm25(&query, top_k).await
        };
        match result {
            Ok(results) => {
                let lines: Vec<String> = results
                    .into_iter()
                    .map(|(id, score)| format!("{}: {:.3}", id, score))
                    .collect();
                let header = format!(
                    "BM25 {}results (req_id: {}, top {}):",
                    if strict { "strict " } else { "" },
                    req_id,
                    top_k
                );
                let content = if lines.is_empty() {
                    format!("{} <no hits>", header)
                } else {
                    format!("{}\n{}", header, lines.join("\n"))
                };
                add_msg(&content).await;
            }
            Err(e) => {
                let msg = format!("BM25 search (req_id: {}) failed: {}", req_id, e);
                add_msg(&msg).await;
            }
        }
    } else {
        let msg = format!(
            "RAG service unavailable; cannot run BM25 search (req_id: {})",
            req_id
        );
        add_msg(&msg).await;
    }
}

pub async fn dense_search(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    req_id: Uuid,
    query: String,
    top_k: usize,
) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

    if let Some(rag) = &state.rag {
        match rag.search(&query, top_k).await {
            Ok(results) => {
                let lines: Vec<String> = results
                    .into_iter()
                    .map(|(id, score)| format!("{}: {:.3}", id, score))
                    .collect();
                let header = format!("Dense results (req_id: {}, top {}):", req_id, top_k);
                let content = if lines.is_empty() {
                    format!("{} <no hits>", header)
                } else {
                    format!("{}\n{}", header, lines.join("\n"))
                };
                add_msg(&content).await;
            }
            Err(e) => {
                let msg = format!("Dense search (req_id: {}) failed: {}", req_id, e);
                add_msg(&msg).await;
            }
        }
    } else {
        let msg = format!(
            "RAG service unavailable; cannot run dense search (req_id: {})",
            req_id
        );
        add_msg(&msg).await;
    }
}

pub async fn assemble_context(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    req_id: Uuid,
    user_query: String,
    top_k: usize,
    budget: &ploke_rag::TokenBudget,
    strategy: ploke_rag::RetrievalStrategy,
) {
    let add_msg = |msg: &str| {
        super::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            crate::chat_history::MessageKind::SysInfo,
        )
    };

    if let Some(rag) = &state.rag {
        match rag.get_context(&user_query, top_k, &budget, strategy).await {
            Ok(_ctx) => {
                let msg = format!(
                    "Assembled context successfully (req_id: {}, top_k: {})",
                    req_id, top_k
                );
                add_msg(&msg).await;
            }
            Err(e) => {
                let msg = format!("Assemble context (req_id: {}) failed: {}", req_id, e);
                add_msg(&msg).await;
            }
        }
    } else {
        let msg = format!(
            "RAG service unavailable; cannot assemble context (req_id: {})",
            req_id
        );
        add_msg(&msg).await;
    }
}
