#![allow(clippy::needless_lifetimes)]
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

/* NOTE: Placeholder until we implement multi-crate parsing and hash the Cargo.toml of the target
   crate, when we will have a real namespace uuid. */
use ploke_core::{WriteSnippetData, PROJECT_NAMESPACE_UUID};
use similar::TextDiff;
use ploke_core::rag_types::{RequestCodeContextArgs, RequestCodeContextResult, GetFileMetadataResult, ApplyCodeEditResult};
use ploke_rag::{RetrievalStrategy, RrfConfig, TokenBudget};

use crate::{app_state::{core::{BeforeAfter, EditProposal, EditProposalStatus, PreviewMode}, handlers::chat}, chat_history::MessageKind};

use super::{editing::approve_edits, utils::{calc_top_k_for_budget, Action, ApplyCodeEditArgs, ToolCallParams, ALLOWED_RELATIONS}, *};

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

    // Parse args (new, concise schema)
    let raw_args = arguments.clone();
    let args: ApplyCodeEditArgs = match serde_json::from_value(raw_args.clone()) {
        Ok(v) => v,
        Err(e) => {
            let err = format!("Invalid apply_code_edit payload: {}", e);
            tool_call_params.tool_call_failed(err);
            return;
        }
    };

    if args.edits.is_empty() {
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
    let mut edits: Vec<WriteSnippetData> = Vec::with_capacity(args.edits.len());
    let mut files_set: BTreeSet<PathBuf> = std::collections::BTreeSet::new();

    for (i, e) in args.edits.iter().enumerate() {
        // Only code_edit supported for now
        match e.action {
            Action::CodeEdit => {}
        }

        // If the edit references a canonical node, resolve via DB as before.
        // If `canon` is empty, allow a direct-file edit shape (file_path + expected_file_hash + start_byte + end_byte + replacement).
        if e.canon.trim().is_empty() {
            // Attempt to read the corresponding raw edit JSON to access alternate key names (e.g., "file_path", "replacement").
            let raw_edit_opt = raw_args
                .get("edits")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.get(i));

            let raw_edit = match raw_edit_opt {
                Some(r) => r,
                None => {
                    tool_call_params
                        .tool_call_failed("Internal error: missing raw edit".to_string());
                    return;
                }
            };

            let file_path_str = raw_edit
                .get("file_path")
                .and_then(|v| v.as_str())
                .or_else(|| raw_edit.get("file").and_then(|v| v.as_str()));
            let file_path_str = match file_path_str {
                Some(s) => s,
                None => {
                    tool_call_params
                        .tool_call_failed("Missing 'file_path' in edit".to_string());
                    return;
                }
            };

            // Compute absolute path (best-effort; prefer crate_root, else absolute or CWD)
            let p = PathBuf::from(file_path_str);
            let abs_path = if p.is_absolute() {
                p
            } else if let Some(root) = crate_root.as_ref() {
                root.join(p)
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(p)
            };

            // Parse expected_file_hash (string UUID) -> TrackingHash
            let expected_hash_str = raw_edit
                .get("expected_file_hash")
                .and_then(|v| v.as_str());
            let expected_file_hash = match expected_hash_str {
                Some(s) => match uuid::Uuid::parse_str(s) {
                    Ok(u) => ploke_core::TrackingHash(u),
                    Err(_) => {
                        let err = format!("Invalid expected_file_hash UUID: {}", s);
                        tool_call_params.tool_call_failed(err);
                        return;
                    }
                },
                None => {
                    tool_call_params
                        .tool_call_failed("Missing expected_file_hash in edit".to_string());
                    return;
                }
            };

            let start_byte = raw_edit
                .get("start_byte")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or(0usize);
            let end_byte = raw_edit
                .get("end_byte")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or(start_byte);

            let replacement = raw_edit
                .get("replacement")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| Some(e.code.clone()))
                .unwrap_or_default();

            let namespace = raw_edit
                .get("namespace")
                .and_then(|v| v.as_str())
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
                .unwrap_or(PROJECT_NAMESPACE_UUID);

            let ws = WriteSnippetData {
                id: uuid::Uuid::new_v4(),
                name: abs_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| abs_path.display().to_string()),
                file_path: abs_path.clone(),
                expected_file_hash,
                start_byte,
                end_byte,
                replacement,
                namespace,
            };
            files_set.insert(abs_path.clone());
            edits.push(ws);
            continue;
        }

        // --- Canonical node resolution path (unchanged) ---

        // Validate relation string (prototype allow-list)
        if !ALLOWED_RELATIONS.contains(&e.node_type.as_str()) {
            let err = format!("Unsupported node_type: {}", e.node_type);
            tool_call_params.tool_call_failed(err);
            return;
        }

        // Compute absolute path (best-effort; prefer crate_root, else absolute or CWD)
        let abs_path = {
            let p = PathBuf::from(&e.file);
            if p.is_absolute() {
                p
            } else if let Some(root) = crate_root.as_ref() {
                root.join(&e.file)
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(&e.file)
            }
        };

        // Canonical path parsing: "module::submodule::Item" -> (["crate","module","submodule"], "Item")
        let canon = e.canon.trim();
        if canon.is_empty() {
            tool_call_params.tool_call_failed("Invalid 'canon': empty".to_string());
            return;
        }
        let (mods_slice, item_name) = match canon.rfind("::") {
            Some(idx) => (&canon[..idx], &canon[idx + 2..]),
            None => ("", canon),
        };
        if item_name.is_empty() {
            tool_call_params.tool_call_failed("Invalid 'canon': missing item name".to_string());
            return;
        }
        // Build module path as &str slices without allocating new Strings
        let mut mod_path: Vec<&str> = Vec::new();
        mod_path.push("crate");
        if !mods_slice.is_empty() {
            mod_path.extend(mods_slice.split("::").filter(|s| !s.is_empty()));
        }

        // PROTOTYPE: parameterless Cozo query with inlined JSON literals (escape-safe via serde_json)
        // WARNING: This relies on exact relation names and NOW snapshots; subject to change.
        let rel = &e.node_type;
        let file_path_lit = serde_json::to_string(&abs_path.to_string_lossy().to_string()).unwrap();
        let item_name_lit = serde_json::to_string(&item_name).unwrap();
        let mod_path_lit = serde_json::to_string(&mod_path).unwrap();

        let script = format!(
            r#"
parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW' }}

ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

?[id, name, file_path, file_hash, hash, span, namespace, mod_path] :=
  *{rel}{{ id, name, tracking_hash: hash, span @ 'NOW' }},
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: mod_path, tracking_hash @ 'NOW' }},
  *file_mod{{ owner_id: mod_id, file_path, namespace @ 'NOW' }},
  name == {item_name_lit},
  file_path == {file_path_lit},
  mod_path == {mod_path_lit}
"#
        );

        let qr = match state.db.raw_query(&script) {
            Ok(q) => q,
            Err(e) => {
                let err = format!("DB query failed: {}", e);
                tool_call_params.tool_call_failed(err.to_string());
                return;
            }
        };

        let mut nodes = match qr.to_embedding_nodes() {
            Ok(v) => v,
            Err(e) => {
                let err = format!("Failed to parse DB result: {}", e);
                tool_call_params.tool_call_failed(err.to_string());
                return;
            }
        };

        if nodes.is_empty() {
            let err = format!(
                "No matching node found for canon={} in file={}",
                e.canon,
                abs_path.display()
            );
            tool_call_params.tool_call_failed(err.to_string());
            return;
        }
        if nodes.len() > 1 {
            // Ambiguity: require model to be more specific (could add 'kind' if needed)
            let err = format!(
                "Ambiguous node resolution ({} candidates) for canon={} in file={}",
                nodes.len(),
                e.canon,
                abs_path.display()
            );
            tool_call_params.tool_call_failed(err.to_string());
            return;
        }

        let ed = nodes.remove(0);
        let ws = WriteSnippetData {
            id: uuid::Uuid::new_v4(),
            name: e.canon.clone(),
            file_path: ed.file_path.clone(),
            expected_file_hash: ed.file_tracking_hash,
            start_byte: ed.start_byte,
            end_byte: ed.end_byte,
            replacement: e.code.clone(),
            namespace: ed.namespace,
        };
        files_set.insert(ed.file_path.clone());
        edits.push(ws);
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
        if matches!(
            editing_cfg.preview_mode,
            PreviewMode::Diff
        ) {
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

    let preview_label = if matches!(
        editing_cfg.preview_mode,
        PreviewMode::Diff
    ) {
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

    let preview_snippet = if matches!(
        editing_cfg.preview_mode,
        PreviewMode::Diff
    ) {
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
        tool_call_params
            .tool_call_failed("Invalid or missing token_budget".to_string());
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
