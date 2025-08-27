#![allow(clippy::needless_lifetimes)]
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

/* NOTE: Placeholder until we implement multi-crate parsing and hash the Cargo.toml of the target
crate, when we will have a real namespace uuid. */
use ploke_core::rag_types::{
    ApplyCodeEditResult, GetFileMetadataResult, RequestCodeContextArgs, RequestCodeContextResult,
};
use ploke_core::{PROJECT_NAMESPACE_UUID, WriteSnippetData};
use ploke_rag::{RetrievalStrategy, RrfConfig, TokenBudget};
use similar::TextDiff;

use crate::{
    app_state::{
        core::{BeforeAfter, EditProposal, EditProposalStatus, PreviewMode},
        handlers::chat,
    },
    chat_history::MessageKind,
};

use super::{
    editing::approve_edits,
    utils::{
        calc_top_k_for_budget, ApplyCodeEditRequest, Edit, LegacyApplyDirect, NodeKind, ALLOWED_RELATIONS,
        ToolCallParams,
    },
    *,
};

pub async fn get_file_metadata_tool<'a>(tool_call_params: ToolCallParams<'a>) {
    let ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        vendor,
        name,
        arguments,
        call_id,
    } = tool_call_params.clone();
    // Validate args
    let Some(file_path_str) = arguments.get("file_path").and_then(|v| v.as_str()) else {
        tool_call_params.tool_call_failed("Missing required argument 'file_path'".to_string());
        return;
    };

    let path = PathBuf::from(file_path_str);
    // Read file and compute a deterministic TrackingHash UUID (v5 over file bytes within project namespace)
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let hash_uuid = uuid::Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &bytes);
            // Get basic metadata
            let (byte_len, modified_ms) = match tokio::fs::metadata(&path).await {
                Ok(md) => {
                    let len = md.len();
                    let modified_ms = md.modified().ok().and_then(|mtime| {
                        mtime
                            .duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_millis() as i64)
                    });
                    (len, modified_ms)
                }
                Err(_) => (bytes.len() as u64, None),
            };

            let result = GetFileMetadataResult {
                ok: true,
                file_path: path.display().to_string(),
                exists: true,
                byte_len,
                modified_ms,
                file_hash: hash_uuid.to_string(),
                tracking_hash: hash_uuid.to_string(),
            };
            let content = match serde_json::to_string(&result) {
                Ok(s) => s,
                Err(e) => {
                    let err = format!("Failed to serialize GetFileMetadataResult: {}", e);
                    tool_call_params.tool_call_failed(err);
                    return;
                }
            };

            let _ = event_bus
                .realtime_tx
                .send(AppEvent::System(SystemEvent::ToolCallCompleted {
                    request_id,
                    parent_id,
                    call_id: call_id.clone(),
                    content,
                }));
        }
        Err(e) => {
            let err = format!("Failed to read file '{}': {}", path.display(), e);
            tool_call_params.tool_call_failed(err);
        }
    }
}

pub async fn apply_code_edit_tool<'a>(tool_call_params: ToolCallParams<'a>) {
    let ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        vendor,
        name,
        arguments,
        call_id,
    } = tool_call_params.clone();
    // Idempotency: guard duplicate requests
    {
        let reg = state.proposals.read().await;
        if reg.contains_key(&request_id) {
            let msg = format!(
                "Duplicate apply_code_edit request ignored for request_id {}",
                request_id
            );
            tool_call_params.tool_call_failed(msg.clone());
            chat::add_msg_immediate(state, event_bus, Uuid::new_v4(), msg, MessageKind::SysInfo)
                .await;
            return;
        }
    }

    // Parse args (strongly typed). Prefer tagged enum request; fall back to legacy direct splice mapping.
    let typed_req: ApplyCodeEditRequest = match serde_json::from_value(arguments.clone()) {
        Ok(v) => v,
        Err(_) => {
            match serde_json::from_value::<LegacyApplyDirect>(arguments.clone()) {
                Ok(legacy) => ApplyCodeEditRequest {
                    confidence: legacy.confidence,
                    edits: legacy
                        .edits
                        .into_iter()
                        .map(|le| Edit::Splice {
                            file_path: le.file_path,
                            expected_file_hash: le.expected_file_hash,
                            start_byte: le.start_byte as u32,
                            end_byte: le.end_byte as u32,
                            replacement: le.replacement,
                            namespace: le.namespace.unwrap_or(PROJECT_NAMESPACE_UUID),
                        })
                        .collect(),
                },
                Err(e2) => {
                    let err = format!("Invalid apply_code_edit payload: {}", e2);
                    tool_call_params.tool_call_failed(err);
                    return;
                }
            }
        }
    };

    if typed_req.edits.is_empty() {
        tool_call_params.tool_call_failed("No edits provided".to_string());
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            "apply_code_edit: No edits provided".to_string(),
            MessageKind::SysInfo,
        )
        .await;
        return;
    }

    // Resolve each edit by canonical path -> EmbeddingData -> WriteSnippetData
    let crate_root = { state.system.read().await.crate_focus.clone() };
    let editing_cfg = { state.config.read().await.editing.clone() };
    let mut edits: Vec<WriteSnippetData> = Vec::with_capacity(typed_req.edits.len());
    let mut files_set: BTreeSet<PathBuf> = std::collections::BTreeSet::new();

    for e in typed_req.edits.iter() {
        match e {
            Edit::Splice { file_path, expected_file_hash, start_byte, end_byte, replacement, namespace } => {
                let p = PathBuf::from(file_path);
                let abs_path = if p.is_absolute() { p } else if let Some(root) = crate_root.as_ref() { root.join(p) } else { std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(p) };
                let ws = WriteSnippetData {
                    id: uuid::Uuid::new_v4(),
                    name: abs_path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| abs_path.display().to_string()),
                    file_path: abs_path.clone(),
                    expected_file_hash: *expected_file_hash,
                    start_byte: *start_byte as usize,
                    end_byte: *end_byte as usize,
                    replacement: replacement.clone(),
                    namespace: *namespace,
                };
                files_set.insert(abs_path.clone());
                edits.push(ws);
            }
            Edit::Canonical { file, canon, node_type, code } => {
                let rel = node_type.as_relation();
                if !ALLOWED_RELATIONS.contains(&rel) {
                    let err = format!("Unsupported node_type: {}", rel);
                    tool_call_params.tool_call_failed(err);
                    return;
                }
                let abs_path = { let p = PathBuf::from(file); if p.is_absolute() { p } else if let Some(root) = crate_root.as_ref() { root.join(file) } else { std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(file) } };
                let canon_trim = canon.trim();
                if canon_trim.is_empty() { tool_call_params.tool_call_failed("Invalid 'canon': empty".to_string()); return; }
                let (mods_slice, item_name) = match canon_trim.rfind("::") { Some(idx) => (&canon_trim[..idx], &canon_trim[idx + 2..]), None => ("", canon_trim), };
                if item_name.is_empty() { tool_call_params.tool_call_failed("Invalid 'canon': missing item name".to_string()); return; }
                let mut mod_path: Vec<&str> = vec!["crate"]; if !mods_slice.is_empty() { mod_path.extend(mods_slice.split("::").filter(|s| !s.is_empty())); }
                let mod_path_owned: Vec<String> = mod_path.iter().map(|s| s.to_string()).collect();
                let mut nodes = match ploke_db::helpers::resolve_nodes_by_canon_in_file(&state.db, rel, &abs_path, &mod_path_owned, item_name) { Ok(v) => v, Err(e) => { let err = format!("DB resolve failed: {}", e); tool_call_params.tool_call_failed(err); return; } };
                if nodes.is_empty() { let err = format!("No matching node found for canon={} in file={}", canon, abs_path.display()); tool_call_params.tool_call_failed(err); return; }
                if nodes.len() > 1 { let err = format!("Ambiguous node resolution ({} candidates) for canon={} in file={}", nodes.len(), canon, abs_path.display()); tool_call_params.tool_call_failed(err); return; }
                let ed = nodes.remove(0);
                let ws = WriteSnippetData { id: uuid::Uuid::new_v4(), name: canon.clone(), file_path: ed.file_path.clone(), expected_file_hash: ed.file_tracking_hash, start_byte: ed.start_byte, end_byte: ed.end_byte, replacement: code.clone(), namespace: ed.namespace };
                files_set.insert(ed.file_path.clone());
                edits.push(ws);
            }
        }
    }

    // Build preview (reuse minimal version from prior implementation)
    let mut per_file: Vec<BeforeAfter> = Vec::new();
    let mut unified_diff = String::new();
    for path in files_set.iter() {
        // Fetch full file content via IoManager (verified against tracking hash)
        let (file_hash, namespace) = edits
            .iter()
            .find(|e| &e.file_path == path)
            .map(|e| (e.expected_file_hash, e.namespace))
            .expect("Mismatched path in file edit");
        let tracking_hash_before = file_hash;
        let before = match state
            .io_handle
            .read_full_verified(path.clone(), file_hash, namespace)
            .await
        {
            Ok(Ok(s)) => s,
            _ => "<unreadable or binary file>".to_string(),
        };
        // Apply all edits for this file in-memory (descending by start to keep indices stable)
        let mut bytes = before.clone().into_bytes();
        let mut file_edits: Vec<&WriteSnippetData> =
            edits.iter().filter(|e| &e.file_path == path).collect();
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
            path.strip_prefix(root)
                .unwrap_or(path.as_path())
                .to_path_buf()
        } else {
            path.clone()
        };
        per_file.push(BeforeAfter {
            file_path: display_path.clone(),
            before: before.clone(),
            after: after.clone(),
        });
        if matches!(editing_cfg.preview_mode, PreviewMode::Diff) {
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

    let files: Vec<PathBuf> = files_set.into_iter().collect();
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

    let preview_label = if matches!(editing_cfg.preview_mode, PreviewMode::Diff) {
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

    let preview_snippet = if matches!(editing_cfg.preview_mode, PreviewMode::Diff) {
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

    let edit_len = edits.len();
    // Stash proposal in registry
    {
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
                preview: if matches!(
                    editing_cfg.preview_mode,
                    crate::app_state::core::PreviewMode::Diff
                ) {
                    crate::app_state::core::DiffPreview::UnifiedDiff {
                        text: unified_diff.clone(),
                    }
                } else {
                    crate::app_state::core::DiffPreview::CodeBlocks {
                        per_file: per_file.clone(),
                    }
                },
                status: EditProposalStatus::Pending,
            },
        );
    }

    // Emit SysInfo summary with how to approve/deny
    let summary = format!(
        r#"Staged code edits (request_id: {request_id}, call_id: {call_id}).
Files:
    {0}

Preview (mode={preview_label}, first {1} lines per section):
{preview_snippet}

Approve:  edit approve {request_id}
Deny:     edit deny {request_id}{2}"#,
        display_files.join("\n  "),
        editing_cfg.max_preview_lines,
        if editing_cfg.auto_confirm_edits {
            "\n\nAuto-approval enabled: applying now..."
        } else {
            ""
        },
    );
    chat::add_msg_immediate(
        state,
        event_bus,
        Uuid::new_v4(),
        summary,
        MessageKind::SysInfo,
    )
    .await;

    // Emit a typed ToolCallCompleted so the LLM loop can proceed deterministically.
    let result = ApplyCodeEditResult {
        ok: true,
        staged: edit_len,
        applied: 0,
        files: display_files.clone(),
        preview_mode: preview_label.to_string(),
        auto_confirmed: editing_cfg.auto_confirm_edits,
    };
    let content = match serde_json::to_string(&result) {
        Ok(s) => s,
        Err(e) => {
            let err = format!("Failed to serialize ApplyCodeEditResult: {}", e);
            tool_call_params.tool_call_failed(err);
            return;
        }
    };
    let _ = event_bus
        .realtime_tx
        .send(AppEvent::System(SystemEvent::ToolCallCompleted {
            request_id,
            parent_id,
            call_id: call_id.clone(),
            content,
        }));

    if editing_cfg.auto_confirm_edits {
        let state2 = Arc::clone(state);
        let event_bus2 = Arc::clone(event_bus);
        tokio::spawn(async move {
            approve_edits(&state2, &event_bus2, request_id).await;
        });
    }
}

pub async fn handle_request_context<'a>(tool_call_params: ToolCallParams<'a>) {
    let ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        vendor: _,
        name: _,
        arguments,
        call_id,
    } = tool_call_params.clone();

    let span = tracing::info_span!("handle_request_context", request_id = %request_id, parent_id = %parent_id, call_id = %call_id);
    let _enter = span.enter();
    tracing::debug!(arguments = ?arguments, "handle_request_context called");

    // Parse typed arguments
    let args: RequestCodeContextArgs = match serde_json::from_value(arguments.clone()) {
        Ok(v) => v,
        Err(e) => {
            let msg = format!("Invalid request_code_context payload: {}", e);
            tool_call_params.tool_call_failed(msg);
            return;
        }
    };
    if args.token_budget == 0 {
        tool_call_params.tool_call_failed("Invalid or missing token_budget".to_string());
        return;
    }

    // Determine query: prefer hint, otherwise last user message
    let query = if let Some(h) = args.hint.as_ref().filter(|s| !s.trim().is_empty()) {
        h.clone()
    } else {
        let guard = state.chat.read().await;
        match guard.last_user_msg() {
            Ok(Some((_id, content))) => content,
            _ => String::new(),
        }
    };

    if query.trim().is_empty() {
        let msg = "No query available (no hint provided and no recent user message)".to_string();
        tool_call_params.tool_call_failed(msg);
        return;
    }

    let top_k = calc_top_k_for_budget(args.token_budget);

    // Build token budget for RAG
    let mut budget = TokenBudget::default();
    budget.max_total = args.token_budget as usize;

    if let Some(rag) = &state.rag {
        match rag
            .get_context(
                &query,
                top_k,
                &budget,
                RetrievalStrategy::Hybrid {
                    rrf: RrfConfig::default(),
                    mmr: None,
                },
            )
            .await
        {
            Ok(context) => {
                let result = RequestCodeContextResult {
                    ok: true,
                    query,
                    top_k,
                    context,
                };
                let content = match serde_json::to_string(&result) {
                    Ok(s) => s,
                    Err(e) => {
                        let msg = format!("Failed to serialize RequestCodeContextResult: {}", e);
                        tool_call_params.tool_call_failed(msg);
                        return;
                    }
                };
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
                let msg = format!("RAG get_context failed: {}", e);
                tracing::warn!("{}", msg);
                tool_call_params.tool_call_failed(msg);
            }
        }
    } else {
        let msg = "RAG service unavailable".to_string();
        tracing::warn!("{}", msg);
        tool_call_params.tool_call_failed(msg);
    }
}
