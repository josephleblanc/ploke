use ploke_core::{PROJECT_NAMESPACE_UUID, TrackingHash, WriteSnippetData};
use ploke_rag::AssembledContext;
use ploke_rag::RagService;
use ploke_rag::RetrievalStrategy;
use ploke_rag::RrfConfig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use similar::TextDiff;
use std::collections::BTreeSet;
use std::collections::{BTreeMap, HashMap};
use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use uuid::Uuid;
use ploke_db::EmbeddingData;

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

use super::chat;
use super::embedding::wait_on_oneshot;

static PROMPT_HEADER: &str = r#"
<-- SYSTEM PROMPT -->
You are a highly skilled software engineer, specializing in the Rust programming language.

You will be asked to provide some assistance in collaborating with the user.
"#;

// TODO: Review the prompt, I think the LLM fucked it up.
static PROMPT_CODE: &str = r#"
Tool-aware code collaboration instructions

You can call tools to request more context and to stage code edits for user approval.
Use the tools ONLY as described below.

Code context format
- You will receive code snippets as system messages. Headers may include:
  - File: absolute path to the file
  - Span: byte or line range the snippet covers
  - File Hash: a UUID identifying the current on-disk content (TrackingHash)
- Snippet hashes and byte ranges are for validation on our side; you should not compute them.

How to request more context
- Call the tool request_code_context when you need additional code to proceed.
- Arguments:
  - token_budget: integer > 0 indicating approximately how many tokens of code to return
  - hint: optional string that narrows the search (file path, symbol name, module, brief description)
- Keep requests focused and iterative.

How to fetch file metadata (TrackingHash)
- If you need file metadata for reasoning, call get_file_metadata with:
  - file_path: absolute path to the file on disk
- The tool returns the file's current tracking hash (UUID), byte length, and modified time.

How to propose code edits (rewrite an entire item)
- Call apply_code_edit with:
  {
    "edits": [
      {
        "action": "code_edit",
        "file": "example_crate/src/main.rs",            // path relative to your project root
        "canon": "module_one::example_module::Thing",   // canonical path of the item (no leading 'crate')
        "node_type": "function",                         // one of: function|struct|enum|trait|type_alias|module|static|const|macro|import|impl|union
        "code": "fn thing() { /* new implementation */ }"
      }
    ]
  }
- Notes:
  - You do NOT provide byte offsets or hashes; we will resolve the canonical path to a node span and validate file hashes internally.
  - Provide complete item definitions (rewrite), including attributes and docs where appropriate.

Examples
- Request context for a file by path:
  request_code_context({ "token_budget": 2000, "hint": "file: crates/ploke-tui/src/app_state/handlers/rag.rs" })
- Request context for a symbol:
  request_code_context({ "token_budget": 1500, "hint": "function: handle_tool_call_requested in rag.rs" })
- Stage a single-item rewrite:
  apply_code_edit({
    "edits": [
      {
        "action": "code_edit",
        "file": "crates/ploke-tui/src/app_state/handlers/rag.rs",
        "canon": "app_state::handlers::rag::handle_tool_call_requested",
        "node_type": "function",
        "code": "pub async fn handle_tool_call_requested(/* ... */) { /* new body */ }"
      }
    ]
  })

Conversation structure
- After the Code section below, the User's query appears under a # USER header.
- If additional responses from collaborators appear (Assistant/Collaborator), treat them as context.
- When uncertain, ask for missing details or request additional context precisely.

# Code

"#;
static PROMPT_USER: &str = r#"
# USER

"#;

#[derive(Debug, Deserialize)]
struct ApplyCodeEditArgs {
    edits: Vec<EditInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    CodeEdit,
    // Create, // not supported yet
}

#[derive(Debug, Deserialize)]
struct EditInput {
    action: Action,
    /// File path relative to the project root (or absolute). Example: "example_crate/src/main.rs"
    file: String,
    /// Canonical path of the target item without leading 'crate'. Example: "module_one::foo::Bar"
    canon: String,
    /// Relation name for the node type. Example: "function", "struct", ...
    node_type: String,
    /// Full rewritten item text (attributes/docs included if applicable)
    code: String,
}

#[derive(Debug, Serialize)]
struct PerEditResult {
    file_path: String,
    ok: bool,
    error: Option<String>,
    new_file_hash: Option<String>,
}

#[derive(Debug, Serialize)]
struct ApplyCodeEditResult {
    ok: bool,
    applied: usize,
    results: Vec<PerEditResult>,
}

const ALLOWED_RELATIONS: &[&str] = &[
    "function",
    "const",
    "enum",
    "impl",
    "import",
    "macro",
    "module",
    "static",
    "struct",
    "trait",
    "type_alias",
    "union",
];

fn json_lit<T: serde::Serialize>(v: &T) -> String {
    // Helper to embed safe JSON literals inside Cozo script (prototype).
    serde_json::to_string(v).unwrap()
}

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

    // Handle atomic code edit application via ploke-io (M2: rewrite by canonical path; staged for approval)
    if name == "apply_code_edit" {
        // Idempotency: guard duplicate requests
        {
            let reg = state.proposals.read().await;
            if reg.contains_key(&request_id) {
                let msg = format!(
                    "Duplicate apply_code_edit request ignored for request_id {}",
                    request_id
                );
                let _ = event_bus
                    .realtime_tx
                    .send(tool_call_failed("Duplicate request_id".to_string()));
                chat::add_msg_immediate(
                    state,
                    event_bus,
                    Uuid::new_v4(),
                    msg,
                    MessageKind::SysInfo,
                )
                .await;
                return;
            }
        }

        // Parse args (new, concise schema)
        let args: ApplyCodeEditArgs = match serde_json::from_value(arguments.clone()) {
            Ok(v) => v,
            Err(e) => {
                let err = format!("Invalid apply_code_edit payload: {}", e);
                let _ = event_bus.realtime_tx.send(tool_call_failed(err));
                return;
            }
        };

        if args.edits.is_empty() {
            let _ = event_bus
                .realtime_tx
                .send(tool_call_failed("No edits provided".to_string()));
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

        for e in &args.edits {
            // Only code_edit supported for now
            match e.action {
                Action::CodeEdit => {}
            }
            // Validate relation string (prototype allow-list)
            if !ALLOWED_RELATIONS.contains(&e.node_type.as_str()) {
                let err = format!("Unsupported node_type: {}", e.node_type);
                let _ = event_bus.realtime_tx.send(tool_call_failed(err));
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
                let _ = event_bus
                    .realtime_tx
                    .send(tool_call_failed("Invalid 'canon': empty".to_string()));
                return;
            }
            let (mods_slice, item_name) = match canon.rfind("::") {
                Some(idx) => (&canon[..idx], &canon[idx + 2..]),
                None => ("", canon),
            };
            if item_name.is_empty() {
                let _ = event_bus
                    .realtime_tx
                    .send(tool_call_failed(
                        "Invalid 'canon': missing item name".to_string(),
                    ));
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
            let file_path_lit = json_lit(&abs_path.to_string_lossy().to_string());
            let item_name_lit = json_lit(&item_name);
            let mod_path_lit = json_lit(&mod_path);

            let script = format!(
                r#"
parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains"}}

ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

?[id, name, file_path, file_hash, hash, span, namespace, mod_path] :=
  *{rel}{{ id, name @ 'NOW', tracking_hash: hash @ 'NOW', span @ 'NOW' }},
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: mod_path @ 'NOW', tracking_hash @ 'NOW' }},
  *file_mod{{ owner_id: mod_id, file_path @ 'NOW', namespace @ 'NOW' }},
  name == {item_name_lit},
  file_path == {file_path_lit},
  mod_path == {mod_path_lit}
"#
            );

            let qr = match state.db.raw_query(&script) {
                Ok(q) => q,
                Err(e) => {
                    let err = format!("DB query failed: {}", e);
                    let _ = event_bus.realtime_tx.send(tool_call_failed(err));
                    return;
                }
            };

            let mut nodes = match qr.to_embedding_nodes() {
                Ok(v) => v,
                Err(e) => {
                    let err = format!("Failed to parse DB result: {}", e);
                    let _ = event_bus.realtime_tx.send(tool_call_failed(err));
                    return;
                }
            };

            if nodes.is_empty() {
                let err = format!(
                    "No matching node found for canon={} in file={}",
                    e.canon,
                    abs_path.display()
                );
                let _ = event_bus.realtime_tx.send(tool_call_failed(err));
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
                let _ = event_bus.realtime_tx.send(tool_call_failed(err));
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

        // Compute a simple args hash for auditing
        // BUG: This is incorrect hashing approach. Should use TrackingHash::generate instead
        let mut hasher = Sha256::new();
        hasher.update(arguments.to_string().as_bytes());
        let args_hash = format!("{:x}", hasher.finalize());

        // Build preview (reuse minimal version from prior implementation)
        let mut per_file: Vec<crate::app_state::core::BeforeAfter> = Vec::new();
        let mut unified_diff = String::new();
        for path in files_set.iter() {
            // Fetch full file content via IoManager (verified against tracking hash)
            let (file_hash, namespace) = edits
                .iter()
                .find(|e| &e.file_path == path)
                .map(|e| (e.expected_file_hash, e.namespace))
                .unwrap_or((TrackingHash(Uuid::nil()), Uuid::nil()));
            let byte_len = match tokio::fs::metadata(path).await {
                Ok(md) => md.len() as usize,
                Err(_) => 0,
            };
            let before = if byte_len > 0 {
                let req = EmbeddingData {
                    file_path: path.clone(),
                    file_tracking_hash: file_hash,
                    node_tracking_hash: file_hash,
                    start_byte: 0,
                    end_byte: byte_len,
                    id: Uuid::new_v4(),
                    name: "full_file".to_string(),
                    namespace,
                };
                match state.io_handle.get_snippets_batch(vec![req]).await {
                    Ok(mut v) => match v.pop() {
                        Some(Ok(s)) => s,
                        _ => "<unreadable or binary file>".to_string(),
                    },
                    Err(_) => "<unreadable or binary file>".to_string(),
                }
            } else {
                "<unreadable or binary file>".to_string()
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
            per_file.push(crate::app_state::core::BeforeAfter {
                file_path: display_path.clone(),
                before: before.clone(),
                after: after.clone(),
            });
            if matches!(
                editing_cfg.preview_mode,
                crate::app_state::core::PreviewMode::Diff
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
            crate::app_state::core::PreviewMode::Diff
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
            crate::app_state::core::PreviewMode::Diff
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

        // Stash proposal in registry
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
            "Staged code edits (request_id: {}, call_id: {}).\nFiles:\n  {}\n\nPreview (mode={}, first {} lines per section):\n{}\n\nApprove:  edit approve {}\nDeny:     edit deny {}{}",
            request_id,
            call_id,
            display_files.join("\n  "),
            preview_label,
            editing_cfg.max_preview_lines,
            preview_snippet,
            request_id,
            request_id,
            if editing_cfg.auto_confirm_edits {
                "\n\nAuto-approval enabled: applying now..."
            } else {
                ""
            }
        );
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            summary,
            MessageKind::SysInfo,
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

    // New: get_file_metadata tool for fetching current file hash and basic metadata
    if name == "get_file_metadata" {
        // Validate args
        let Some(file_path_str) = arguments.get("file_path").and_then(|v| v.as_str()) else {
            let _ = event_bus
                .realtime_tx
                .send(tool_call_failed("Missing required argument 'file_path'".to_string()));
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

                let content = serde_json::json!({
                    "ok": true,
                    "file_path": path.display().to_string(),
                    "exists": true,
                    "byte_len": byte_len,
                    "modified_ms": modified_ms,
                    "file_hash": hash_uuid.to_string(),
                    "tracking_hash": hash_uuid.to_string(),
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
                let err = format!("Failed to read file '{}': {}", path.display(), e);
                let _ = event_bus
                    .realtime_tx
                    .send(tool_call_failed(err));
            }
        }
        return;
    }

    if name != "request_code_context" {
        tracing::warn!("Unsupported tool call: {}", name);
        let err = format!("Unsupported tool: {}", name);
        let _ = event_bus.realtime_tx.send(tool_call_failed(err.clone()));
        return;
    }

    // Parse arguments
    let token_budget = arguments
        .get("token_budget")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32);
    if token_budget.is_none() || token_budget == Some(0) {
        let msg = "Invalid or missing token_budget".to_string();
        let _ = event_bus.realtime_tx.send(tool_call_failed(msg.clone()));
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
        let _ = event_bus.realtime_tx.send(tool_call_failed(msg.clone()));
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
                let _ = event_bus.realtime_tx.send(tool_call_failed(msg));
            }
        }
    } else {
        let msg = "RAG service unavailable".to_string();
        tracing::warn!("{}", msg);
        let _ = event_bus.realtime_tx.send(tool_call_failed(msg.clone()));
    }
}

pub async fn approve_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    use crate::app_state::core::EditProposalStatus;
    let mut reg = state.proposals.write().await;
    let Some(mut proposal) = reg.get(&request_id).cloned() else {
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            format!(
                "No staged edit proposal found for request_id {}",
                request_id
            ),
            MessageKind::SysInfo,
        )
        .await;
        return;
    };

    // Idempotency checks
    match proposal.status {
        EditProposalStatus::Pending => { /* ok */ }
        EditProposalStatus::Applied => {
            chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Edits already applied for request_id {}", request_id),
                MessageKind::SysInfo,
            )
            .await;
            return;
        }
        EditProposalStatus::Denied => {
            chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Edits already denied for request_id {}", request_id),
                MessageKind::SysInfo,
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

            chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Failed to apply edits for request_id {}: {}", request_id, e),
                MessageKind::SysInfo,
            )
            .await;
        }
    }
}

pub async fn deny_edits(state: &Arc<AppState>, event_bus: &Arc<EventBus>, request_id: Uuid) {
    use crate::app_state::core::EditProposalStatus;
    let mut reg = state.proposals.write().await;
    let Some(mut proposal) = reg.get(&request_id).cloned() else {
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            format!(
                "No staged edit proposal found for request_id {}",
                request_id
            ),
            MessageKind::SysInfo,
        )
        .await;
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

            chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Denied edits for request_id {}", request_id),
                MessageKind::SysInfo,
            )
            .await;
        }
        EditProposalStatus::Denied => {
            chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Edits already denied for request_id {}", request_id),
                MessageKind::SysInfo,
            )
            .await;
        }
        EditProposalStatus::Applied => {
            chat::add_msg_immediate(
                state,
                event_bus,
                Uuid::new_v4(),
                format!("Edits already applied for request_id {}", request_id),
                MessageKind::SysInfo,
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
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
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
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
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
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
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
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
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
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
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
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
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
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
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
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
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
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
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
        chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
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
