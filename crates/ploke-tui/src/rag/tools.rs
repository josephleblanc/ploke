#![allow(clippy::needless_lifetimes)]
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

/* NOTE: Placeholder until we implement multi-crate parsing and hash the Cargo.toml of the target
crate, when we will have a real namespace uuid. */
use ploke_core::rag_types::{
    ApplyCodeEditResult, GetFileMetadataResult, RequestCodeContextArgs, RequestCodeContextResult,
};
use ploke_core::{ArcStr, FileData, PROJECT_NAMESPACE_UUID, TrackingHash, WriteSnippetData};
use ploke_db::NodeType;
use ploke_error::{DomainError, InternalError};
use ploke_io::read::{FileHashData, read_and_compute_filehash};
use ploke_io::{Diff, NsWriteSnippetData, ReadStrategy};
use ploke_rag::{RetrievalStrategy, RrfConfig, TokenBudget};
use similar::TextDiff;
use tracing::debug;

use crate::tools::create_file::CreateFileCtx;
use crate::tools::{ToolName, ToolUiPayload};
use crate::utils::path_scoping;
use crate::{
    app_state::{
        core::{BeforeAfter, CreateProposal, EditProposal, EditProposalStatus, PreviewMode},
        handlers::chat,
    },
    chat_history::MessageKind,
};

use super::{
    editing::approve_edits,
    utils::{
        ApplyCodeEditRequest, Edit, LegacyApplyDirect, NodeKind, ToolCallParams,
        calc_top_k_for_budget,
    },
    *,
};

pub trait ToolInput<T>
where
    T: Send + Sync + Clone + Serialize + for<'a> Deserialize<'a>,
{
    fn request_id(&self) -> Uuid;
    fn parent_id(&self) -> Uuid;
    fn arguments(self) -> T;
    fn arguments_ref(&self) -> &T;
}
pub trait ToolOutput {}

pub(crate) trait LlmTool<T, R, U>
where
    T: Send + Sync + Clone + Serialize + for<'sea> Deserialize<'sea> + ToolInput<U>,
    R: Send + Sync + Clone + Serialize + Deserialize<'static> + ToolOutput,
    U: Send + Sync + Clone + Serialize + for<'sec> Deserialize<'sec>,
{
    async fn call_tool(&self, tool_input: T) -> R;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetContextInput {
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub call_id: ArcStr,

    pub arguments: serde_json::Value,
}

impl ToolInput<serde_json::Value> for GetContextInput {
    fn request_id(&self) -> Uuid {
        self.request_id
    }

    fn parent_id(&self) -> Uuid {
        self.parent_id
    }

    fn arguments(self) -> serde_json::Value {
        self.arguments
    }

    fn arguments_ref(&self) -> &serde_json::Value {
        &self.arguments
    }
}

pub async fn apply_code_edit_tool(tool_call_params: ToolCallParams) {
    let ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        name,
        typed_req,
        call_id,
    } = tool_call_params.clone();
    if let Some(parse_failure) = state.system.read().await.last_parse_failure() {
        tool_call_params.tool_call_failed(parse_failure.message.clone());
        return;
    }
    // Idempotency: guard duplicate requests
    {
        let reg = state.proposals.read().await;
        if reg.contains_key(&request_id) {
            let msg = format!(
                "Duplicate apply_code_edit request ignored for request_id {}",
                request_id
            );
            #[cfg(test)]
            eprintln!("Detected duplicate for state.proposals.read");
            tool_call_params.tool_call_failed(msg.clone());
            chat::add_msg_immediate(
                &state,
                &event_bus,
                Uuid::new_v4(),
                msg,
                MessageKind::SysInfo,
            )
            .await;
            return;
        }
    }

    if typed_req.edits.is_empty() {
        tool_call_params.tool_call_failed("No edits provided".to_string());
        chat::add_msg_immediate(
            &state,
            &event_bus,
            Uuid::new_v4(),
            "apply_code_edit: No edits provided".to_string(),
            MessageKind::SysInfo,
        )
        .await;
        return;
    }

    // Resolve each edit by canonical path -> EmbeddingData -> WriteSnippetData
    let crate_root = { state.system.read().await.focused_crate_root() };
    let editing_cfg = { state.config.read().await.editing.clone() };
    let mut edits: Vec<WriteSnippetData> = Vec::with_capacity(typed_req.edits.len());
    let mut files_set: BTreeSet<PathBuf> = std::collections::BTreeSet::new();

    for edit in typed_req.edits.iter() {
        match edit {
            Edit::Splice {
                file_path,
                expected_file_hash,
                start_byte,
                end_byte,
                replacement,
                namespace,
            } => {
                let p = PathBuf::from(file_path);
                let abs_path = if let Some(root) = crate_root.as_ref() {
                    match crate::utils::path_scoping::resolve_in_crate_root(&p, root) {
                        Ok(pb) => pb,
                        Err(err) => {
                            let msg = format!("invalid path: {}", err);
                            tool_call_params.tool_call_failed(msg);
                            return;
                        }
                    }
                } else if p.is_absolute() {
                    p
                } else {
                    std::env::current_dir()
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .join(p)
                };
                let ws = WriteSnippetData {
                    id: uuid::Uuid::new_v4(),
                    name: abs_path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| abs_path.display().to_string()),
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
            Edit::Canonical {
                file,
                canon,
                node_type,
                code,
            } => {
                if !NodeType::primary_nodes().contains(node_type) {
                    let err = format!(
                        "Unsupported node type '{}': only primary_nodes() are supported for code editing",
                        node_type.relation_str()
                    );
                    tool_call_params.tool_call_failed(err);
                    return;
                }
                // TODO: Clean up the next 20 lines or so
                let p = PathBuf::from(file);
                let file_was_relative = !p.is_absolute();
                let abs_path = if let Some(root) = crate_root.as_ref() {
                    match crate::utils::path_scoping::resolve_in_crate_root(&p, root) {
                        Ok(pb) => pb,
                        Err(err) => {
                            let msg = format!("invalid path: {}", err);
                            tool_call_params.tool_call_failed(msg);
                            return;
                        }
                    }
                } else if p.is_absolute() {
                    p
                } else {
                    std::env::current_dir()
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .join(p)
                };
                let canon_trim = canon.trim();
                if canon_trim.is_empty() {
                    tool_call_params.tool_call_failed("Invalid 'canon': empty".to_string());
                    return;
                }
                let (mods_slice, item_name) = match canon_trim.rfind("::") {
                    Some(idx) => (&canon_trim[..idx], &canon_trim[idx + 2..]),
                    None => ("", canon_trim),
                };
                if item_name.is_empty() {
                    tool_call_params
                        .tool_call_failed("Invalid 'canon': missing item name".to_string());
                    return;
                }
                let mod_path_owned: Vec<String> = if mods_slice.is_empty() {
                    vec!["crate".to_string()]
                } else {
                    let segs = mods_slice
                        .split("::")
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>();
                    if segs.first().map(|s| s.as_str()) != Some("crate") {
                        let mut with_crate = Vec::with_capacity(segs.len() + 1);
                        with_crate.push("crate".to_string());
                        with_crate.extend(segs.into_iter());
                        with_crate
                    } else {
                        segs
                    }
                };
                let mut nodes = match ploke_db::helpers::graph_resolve_exact(
                    &state.db,
                    node_type.relation_str(),
                    &abs_path,
                    &mod_path_owned,
                    item_name,
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        let err = format!("DB resolve failed: {}", e);
                        tool_call_params.tool_call_failed(err);
                        return;
                    }
                };
                if nodes.is_empty() {
                    // Fallback: relaxed resolver (module-only) with post-filtering by normalized file path
                    let cands = match ploke_db::helpers::resolve_nodes_by_canon(
                        &state.db,
                        node_type.relation_str(),
                        &mod_path_owned,
                        item_name,
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            let err = format!("DB relaxed resolve failed: {}", e);
                            tool_call_params.tool_call_failed(err);
                            return;
                        }
                    };
                    let filtered: Vec<ploke_core::io_types::EmbeddingData> = cands
                        .into_iter()
                        .filter(|ed| {
                            if ed.file_path == abs_path {
                                true
                            } else if file_was_relative {
                                ed.file_path.to_string_lossy().ends_with(file)
                            } else {
                                false
                            }
                        })
                        .collect();
                    if filtered.is_empty() {
                        let cfiles: Vec<String> = filtered
                            .iter()
                            .map(|ed| ed.file_path.display().to_string())
                            .collect();
                        let err = format!(
                            "No matching node found (strict+fallback) for canon={} in file={}; candidates files={:?}",
                            canon,
                            abs_path.display(),
                            cfiles
                        );
                        tool_call_params.tool_call_failed(err);
                        return;
                    }
                    if filtered.len() > 1 {
                        let cfiles: Vec<String> = filtered
                            .iter()
                            .map(|ed| ed.file_path.display().to_string())
                            .collect();
                        let err = format!(
                            "Ambiguous node resolution ({} candidates after fallback) for canon={} in file={}; candidates files={:?}",
                            filtered.len(),
                            canon,
                            abs_path.display(),
                            cfiles
                        );
                        tool_call_params.tool_call_failed(err);
                        return;
                    }
                    nodes = filtered;
                }
                if nodes.len() > 1 {
                    let err = format!(
                        "Ambiguous node resolution ({} candidates) for canon={} in file={}",
                        nodes.len(),
                        canon,
                        abs_path.display()
                    );
                    tool_call_params.tool_call_failed(err);
                    return;
                }
                let ed = nodes.remove(0);
                let ws = WriteSnippetData {
                    id: uuid::Uuid::new_v4(),
                    name: canon.clone(),
                    file_path: ed.file_path.clone(),
                    expected_file_hash: ed.file_tracking_hash,
                    start_byte: ed.start_byte,
                    end_byte: ed.end_byte,
                    replacement: code.clone(),
                    namespace: ed.namespace,
                };
                files_set.insert(ed.file_path.clone());
                edits.push(ws);
            }
            Edit::Patch { .. } => {
                tracing::trace!("Patch found in apply_code_edit_tool call");
            }
        }
    }

    // Build preview (reuse minimal version from prior implementation)
    let mut per_file: Vec<BeforeAfter> = Vec::new();
    let mut unified_diff = String::new();

    // Define truncate function early so it can be used for stored content
    let truncate = |s: &str| -> String {
        let max = editing_cfg.max_preview_lines;
        let mut out = String::new();
        for (i, line) in s.lines().enumerate() {
            if i >= max {
                out.push_str("... [truncated]");
                break;
            }
            if i > 0 {
                out.push('\n');
            }
            out.push_str(line);
        }
        out
    };
    for path in files_set.iter() {
        // Fetch full file content via IoManager (verified against tracking hash)
        let (file_hash, namespace) = edits
            .iter()
            .find(|e| &e.file_path == path)
            .map(|e| (e.expected_file_hash, e.namespace))
            .expect("Mismatched path in file edit");
        let tracking_hash_before = file_hash;
        // Read via IoManager with tracking-hash verification; fall back to a placeholder on error.
        let before = match state
            .io_handle
            .read_full_verified(path.clone(), file_hash, namespace)
            .await
        {
            Ok(Ok(s)) => s,
            _ => "<unreadable or binary file>".to_string(),
        };
        tracing::debug!(?before);
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
            before: truncate(&before),
            after: truncate(&after),
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
                // TODO: Change edits_ns to a None or otherwise handle better
                edits_ns: Vec::new(),
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
                is_semantic: true,
            },
        );
    }
    // Persist proposals (best-effort)
    crate::app_state::handlers::proposals::save_proposals(&state).await;

    // Emit SysInfo summary with how to approve/deny
    let summary = format!(
        r#"Staged code edits (request_id: {request_id}, call_id: {call_id:?}).
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
        &state,
        &event_bus,
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
    let ui_payload = ToolUiPayload::new(
        ToolName::ApplyCodeEdit,
        call_id.clone(),
        format!(
            "Staged {} edits across {} files",
            result.staged,
            result.files.len()
        ),
    )
    .with_request_id(request_id)
    .with_field("staged", result.staged.to_string())
    .with_field("applied", result.applied.to_string())
    .with_field("files", result.files.len().to_string())
    .with_field("preview_mode", result.preview_mode.as_str())
    .with_field("auto_confirmed", result.auto_confirmed.to_string());
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
            ui_payload: Some(ui_payload),
        }));

    if editing_cfg.auto_confirm_edits {
        let state2 = Arc::clone(&state);
        let event_bus2 = Arc::clone(&event_bus);
        tokio::spawn(async move {
            approve_edits(&state2, &event_bus2, request_id).await;
        });
    }
}

pub async fn apply_ns_code_edit_tool(
    tool_call_params: ToolCallParams,
) -> Result<(), ploke_error::Error> {
    let ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        name,
        typed_req,
        call_id,
    } = tool_call_params.clone();
    let crate_root = { state.system.read().await.focused_crate_root() };
    let editing_cfg = { state.config.read().await.editing.clone() };
    let edits: Vec<WriteSnippetData> = Vec::with_capacity(typed_req.edits.len());
    let files_set: BTreeSet<PathBuf> = std::collections::BTreeSet::new();
    let count = typed_req.edits.len();
    let mut patches = typed_req.edits.into_iter().filter_map(|ed| match ed {
        Edit::Patch {
            file,
            diff,
            reasoning,
        } => Some((file, diff, reasoning)),
        _ => None,
    });
    if count > 1 {
        tracing::error!("found multiple patches in apply_ns_code_edit_tool\ncount: {count}");
    }
    if let Some((file, diff, reasoning)) = patches.next() {
        use mpatch::ApplyOptions;
        let state_cfg = state.config.read().await;
        let apply_options = ApplyOptions::from(state_cfg.editing.patch_cfg);

        // let p = PathBuf::from(file);
        // let file_was_relative = !p.is_absolute();
        // let abs_path = if let Some(root) = crate_root.as_ref() {
        //     match crate::utils::path_scoping::resolve_in_crate_root(&p, root) {
        //         Ok(pb) => pb,
        //         Err(err) => {
        //             let msg = format!("invalid path: {}", err);
        //             tool_call_params.tool_call_failed(msg);
        //             return;
        //         }
        //     }
        // } else if p.is_absolute() {
        //     p
        // } else {
        //     std::env::current_dir()
        //         .unwrap_or_else(|_| PathBuf::from("."))
        //         .join(p)
        // };

        let crate_root = state
            .system
            .read()
            .await
            .focused_crate_root()
            .ok_or_else(|| {
                ploke_error::Error::Domain(DomainError::Ui {
                    message:
                        "No crate is currently focused; load a workspace before using read_file."
                            .to_string(),
                })
            })?;

        let requested_path = PathBuf::from(file.as_str());
        let abs_path =
            path_scoping::resolve_in_crate_root(&requested_path, &crate_root).map_err(|err| {
                ploke_error::Error::Domain(DomainError::Io {
                    message: format!("invalid path: {err}"),
                })
            })?;

        let request = ploke_io::ReadFileRequest {
            file_path: abs_path.clone(),
            range: None,
            max_bytes: None,
            strategy: ReadStrategy::Plain,
        };

        let read_resp = state.io_handle.read_file(request).await.map_err(|err| {
            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                "io channel error: {err}"
            )))
        })??;

        let ploke_io::ReadFileResponse {
            exists,
            file_path,
            byte_len,
            content: maybe_content,
            truncated: io_truncated,
            file_hash,
        } = read_resp;
        let content = maybe_content.ok_or_else(|| {
            let msg = format!(
                "failed to unwrap content for file {:?}",
                file_path.to_string_lossy()
            );
            tool_call_params.tool_call_failed(msg.clone());
            ploke_error::Error::Internal(ploke_error::InternalError::NotImplemented(msg))
        })?;

        // let (content, slice_truncated) = match content {
        //     Some(src) => {
        //         let (sliced, truncated) = slice_content_lines(src, start_line, end_line);
        //         (Some(sliced), truncated)
        //     }
        //     None => (None, false),
        // };

        debug!(?abs_path);

        use mpatch::{apply_patches_to_dir, parse_auto};
        let patches = parse_auto(&diff).map_err(|e| {
            let msg = e.to_string();
            tool_call_params.tool_call_failed(msg.clone());
            ploke_error::Error::Internal(ploke_error::InternalError::NotImplemented(msg))
        })?;

        // Pick a namespace. If you really don’t have one yet, re-use your placeholder.
        let namespace = PROJECT_NAMESPACE_UUID;

        let patch_result = mpatch::parse_single_patch(&diff).map_err(|e| {
            let msg = format!("invalid unified diff: {}", e);
            tool_call_params.tool_call_failed(msg.clone());
            ploke_error::Error::Internal(ploke_error::InternalError::NotImplemented(msg))
        });
        let patch = patch_result.map_err(|e| {
            let msg = format!("failed to patch {}: {}", abs_path.display(), e);
            tool_call_params.tool_call_failed(msg.clone());
            ploke_error::Error::Internal(ploke_error::InternalError::NotImplemented(msg))
        })?;

        let unified_diff = patch.hunks.clone().into_iter().map(|h| h.to_string()).fold(
            String::new(),
            |mut acc, s| {
                acc.push_str(&s);
                acc
            },
        );

        let apply_patch_result =
            mpatch::apply_patch_to_content(&patch, Some(&content), &apply_options);
        let per_file = BeforeAfter {
            file_path: abs_path,
            before: content,
            after: apply_patch_result.new_content,
        };
        let options = editing_cfg.patch_cfg;
        let large_file_policy = editing_cfg.large_file_policy;
        let sn_write_data = NsWriteSnippetData {
            id: request_id,
            file_path,
            expected_file_hash: file_hash,
            namespace,
            diff: Diff::from(diff),
            options,
            large_file_policy,
        };
        let edits_ns: Vec<NsWriteSnippetData> = vec![sn_write_data];
        let files = Vec::new();
        let mut reg = state.proposals.write().await;
        reg.insert(
            request_id,
            EditProposal {
                request_id,
                parent_id,
                call_id: call_id.clone(),
                proposed_at_ms: chrono::Utc::now().timestamp_millis(),
                edits,
                edits_ns,
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
                        per_file: vec![per_file.clone()],
                    }
                },
                status: EditProposalStatus::Pending,
                is_semantic: false,
            },
        );
    }
    Ok(())
}
