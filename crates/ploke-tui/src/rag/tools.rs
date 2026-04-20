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
use similar::{ChangeTag, TextDiff};
use tracing::debug;

use crate::tools::create_file::CreateFileCtx;
use crate::tools::{ToolError, ToolErrorCode, ToolName, ToolUiPayload};
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

pub async fn stage_semantic_edit_proposal(
    tool_call_params: ToolCallParams,
    edits: Vec<WriteSnippetData>,
) {
    let ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        name,
        call_id,
        ..
    } = tool_call_params.clone();

    {
        let reg = state.proposals.read().await;
        if reg.contains_key(&request_id) {
            let msg = format!(
                "Duplicate {} request ignored for request_id {}",
                name.as_str(),
                request_id
            );
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

    if edits.is_empty() {
        tool_call_params.tool_call_failed("No edits provided".to_string());
        return;
    }

    let primary_root = state
        .with_system_read(|sys| sys.tool_path_context().map(|(p, _)| p.clone()))
        .await;
    let editing_cfg = { state.config.read().await.editing.clone() };
    let files_set: BTreeSet<PathBuf> = edits.iter().map(|edit| edit.file_path.clone()).collect();
    let mut per_file: Vec<BeforeAfter> = Vec::new();
    let mut unified_diff = String::new();
    let mut chat_preview_sections: Vec<String> = Vec::new();

    for path in &files_set {
        let (file_hash, namespace) = edits
            .iter()
            .find(|e| &e.file_path == path)
            .map(|e| (e.expected_file_hash, e.namespace))
            .expect("Mismatched path in file edit");
        let before = match state
            .io_handle
            .read_full_verified(path.clone(), file_hash, namespace)
            .await
        {
            Ok(Ok(s)) => s,
            _ => "<unreadable or binary file>".to_string(),
        };
        tracing::debug!(?before);

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

        let display_path = if let Some(root) = primary_root.as_ref() {
            path.strip_prefix(root)
                .unwrap_or(path.as_path())
                .to_path_buf()
        } else {
            path.clone()
        };
        per_file.push(BeforeAfter {
            file_path: display_path.clone(),
            before: truncate_lines(&before, editing_cfg.max_preview_lines),
            after: truncate_lines(&after, editing_cfg.max_preview_lines),
        });
        chat_preview_sections.push(diff_chunk_with_context(
            &display_path,
            &before,
            &after,
            CHAT_PREVIEW_CONTEXT_LINES,
        ));
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
            if let Some(root) = primary_root.as_ref() {
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
    let chat_preview_snippet = if matches!(editing_cfg.preview_mode, PreviewMode::Diff) {
        let filtered = filter_unified_diff_with_context(&unified_diff, CHAT_PREVIEW_CONTEXT_LINES);
        truncate_lines(&filtered, editing_cfg.max_preview_lines)
    } else {
        truncate_lines(
            &chat_preview_sections.join("\n"),
            editing_cfg.max_preview_lines,
        )
    };

    let edit_len = edits.len();
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
    crate::app_state::handlers::proposals::save_proposals(&state).await;

    let summary = format!(
        r#"Staged code edits (request_id: {request_id}, call_id: {call_id:?}).
Files:
    {files}

Preview (mode={preview_label}, context={context_lines} lines, first {max_lines} lines):
{preview_snippet}

Approve:  edit approve {request_id}
Deny:     edit deny {request_id}{auto_confirm}"#,
        files = display_files.join("\n  "),
        preview_label = preview_label,
        context_lines = CHAT_PREVIEW_CONTEXT_LINES,
        max_lines = editing_cfg.max_preview_lines,
        preview_snippet = chat_preview_snippet,
        auto_confirm = if editing_cfg.auto_confirm_edits {
            "\n\nAuto-approval enabled: applying now..."
        } else {
            ""
        },
    );
    chat::add_msg_immediate_sysinfo_unpinned(&state, &event_bus, Uuid::new_v4(), summary).await;

    let result = ApplyCodeEditResult {
        ok: true,
        staged: edit_len,
        applied: 0,
        files: display_files.clone(),
        preview_mode: preview_label.to_string(),
        auto_confirmed: editing_cfg.auto_confirm_edits,
    };
    let ui_payload = ToolUiPayload::new(
        name,
        call_id.clone(),
        format!(
            "Staged {} edits across {} files",
            result.staged,
            result.files.len()
        ),
    )
    .with_request_id(request_id)
    .with_field("status", "pending")
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

const CHAT_PREVIEW_CONTEXT_LINES: usize = 2;

fn truncate_lines(text: &str, max_lines: usize) -> String {
    let mut out = String::new();
    for (i, line) in text.lines().enumerate() {
        if i >= max_lines {
            out.push_str("... [truncated]");
            break;
        }
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

fn filter_unified_diff_with_context(text: &str, context_lines: usize) -> String {
    if context_lines == 0 {
        return text.to_string();
    }

    let lines: Vec<&str> = text.lines().collect();
    let mut keep = vec![false; lines.len()];

    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("diff --git")
            || line.starts_with("---")
            || line.starts_with("+++")
            || line.starts_with("@@")
        {
            keep[i] = true;
            continue;
        }

        let is_add = line.starts_with('+') && !line.starts_with("+++");
        let is_del = line.starts_with('-') && !line.starts_with("---");
        if is_add || is_del {
            let start = i.saturating_sub(context_lines);
            let end = (i + context_lines).min(lines.len().saturating_sub(1));
            for j in start..=end {
                keep[j] = true;
            }
        }
    }

    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if keep[i] {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn diff_chunk_with_context(
    path: &PathBuf,
    before: &str,
    after: &str,
    context_lines: usize,
) -> String {
    let diff = TextDiff::from_lines(before, after);
    let mut out = String::new();
    out.push_str(&format!("--- {}\n", path.display()));

    let mut changes: Vec<(ChangeTag, &str)> = Vec::new();
    for change in diff.iter_all_changes() {
        changes.push((change.tag(), change.value()));
    }

    let mut keep = vec![false; changes.len()];
    for (i, (tag, _)) in changes.iter().enumerate() {
        if *tag == ChangeTag::Equal {
            continue;
        }
        let start = i.saturating_sub(context_lines);
        let end = (i + context_lines).min(changes.len().saturating_sub(1));
        for j in start..=end {
            keep[j] = true;
        }
    }

    for (i, (tag, value)) in changes.iter().enumerate() {
        if !keep[i] {
            continue;
        }

        let prefix = match tag {
            ChangeTag::Delete => '-',
            ChangeTag::Insert => '+',
            ChangeTag::Equal => ' ',
        };

        out.push(prefix);
        out.push(' ');
        out.push_str(value.strip_suffix('\n').unwrap_or(value));
        out.push('\n');
    }
    out
}

#[derive(Debug, Clone)]
enum SemanticCanonTarget {
    Primary {
        module_path: Vec<String>,
        item_name: String,
    },
    Method {
        module_path: Vec<String>,
        owner_name: String,
        item_name: String,
    },
}

impl SemanticCanonTarget {
    fn module_path(&self) -> &[String] {
        match self {
            Self::Primary { module_path, .. } | Self::Method { module_path, .. } => module_path,
        }
    }

    fn item_name(&self) -> &str {
        match self {
            Self::Primary { item_name, .. } | Self::Method { item_name, .. } => item_name,
        }
    }

    fn owner_name(&self) -> Option<&str> {
        match self {
            Self::Primary { .. } => None,
            Self::Method { owner_name, .. } => Some(owner_name),
        }
    }
}

fn split_canon_for_semantic_target(
    canon: &str,
    node_type: NodeType,
) -> Result<SemanticCanonTarget, String> {
    let canon_trim = canon.trim();
    if canon_trim.is_empty() {
        return Err("Invalid 'canon': empty".to_string());
    }

    let mut segs = canon_trim
        .split("::")
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    if segs.first().copied() != Some("crate") {
        segs.insert(0, "crate");
    }

    match node_type {
        NodeType::Method => {
            if segs.len() < 4 {
                return Err(
                    "Invalid 'canon': method targets must look like crate::module::Type::method"
                        .to_string(),
                );
            }
            let item_name = segs.last().expect("checked len").to_string();
            let owner_name = segs[segs.len() - 2].to_string();
            let module_path = segs[..segs.len() - 2]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            if owner_name.is_empty() {
                return Err("Invalid 'canon': missing method owner type".to_string());
            }
            if item_name.is_empty() {
                return Err("Invalid 'canon': missing item name".to_string());
            }
            Ok(SemanticCanonTarget::Method {
                module_path,
                owner_name,
                item_name,
            })
        }
        _ => {
            if segs.len() < 2 {
                return Err("Invalid 'canon': missing item name".to_string());
            }
            let item_name = segs.last().expect("checked len").to_string();
            let module_path = segs[..segs.len() - 1]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            if item_name.is_empty() {
                return Err("Invalid 'canon': missing item name".to_string());
            }
            Ok(SemanticCanonTarget::Primary {
                module_path,
                item_name,
            })
        }
    }
}

fn function_to_method_hint(
    tool: ToolName,
    canon: &str,
    file_path: &PathBuf,
    module_path: &[String],
    owner_name: Option<&str>,
    item_name: &str,
) -> ToolError {
    let retry_context = serde_json::json!({
        "requested_node_type": "function",
        "suggested_node_type": "method",
        "file_path": file_path.display().to_string(),
        "canon": canon,
        "module_path": module_path,
        "owner_name": owner_name,
        "item_name": item_name,
        "reason": "unique method target exists at the same coordinates",
    });

    ToolError::new(
        tool,
        ToolErrorCode::WrongType,
        format!(
            "No matching function target found for canon={} in file={}; the same coordinates resolve uniquely as a method target.",
            canon,
            file_path.display()
        ),
    )
    .field("node_type")
    .expected("method")
    .received("function")
    .retry_hint("Retry with node_type=method for this canonical path.")
    .retry_context(retry_context)
}

fn ambiguous_method_target_error(
    tool: ToolName,
    canon: &str,
    file_path: &PathBuf,
    module_path: &[String],
    owner_name: Option<&str>,
    item_name: &str,
    candidate_count: usize,
) -> ToolError {
    let retry_context = serde_json::json!({
        "requested_node_type": "method",
        "file_path": file_path.display().to_string(),
        "canon": canon,
        "module_path": module_path,
        "owner_name": owner_name,
        "item_name": item_name,
        "candidate_count": candidate_count,
        "reason": "multiple method targets matched after corrected method parsing",
    });

    ToolError::new(
        tool,
        ToolErrorCode::InvalidFormat,
        format!(
            "Ambiguous method target for canon={} in file={}; {} candidates matched after method parsing.",
            canon,
            file_path.display(),
            candidate_count
        ),
    )
    .field("canon")
    .retry_hint("Disambiguate the owner type or use a more specific canonical path.")
    .retry_context(retry_context)
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
    if let Some(parse_failure) = state
        .with_system_read(|sys| sys.last_parse_failure().cloned())
        .await
    {
        tool_call_params.tool_call_failed(parse_failure.message.clone());
        return;
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
    let tool_paths = state
        .with_system_read(|sys| {
            sys.tool_path_context()
                .map(|(p, pol)| (p.clone(), pol.clone()))
        })
        .await;
    let mut edits: Vec<WriteSnippetData> = Vec::with_capacity(typed_req.edits.len());

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
                let abs_path = match &tool_paths {
                    Some((primary, policy)) => {
                        match path_scoping::resolve_tool_path(p.as_path(), primary, policy) {
                            Ok(pb) => pb,
                            Err(err) => {
                                let msg = format!("invalid path: {}", err);
                                tool_call_params.tool_call_failed(msg);
                                return;
                            }
                        }
                    }
                    None => {
                        if p.is_absolute() {
                            p
                        } else {
                            std::env::current_dir()
                                .unwrap_or_else(|_| PathBuf::from("."))
                                .join(p)
                        }
                    }
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
                edits.push(ws);
            }
            Edit::Canonical {
                file,
                canon,
                node_type,
                code,
            } => {
                if !NodeType::primary_and_assoc_nodes().contains(node_type) {
                    let err = format!(
                        "Unsupported node type '{}': only primary_and_assoc_nodes() are supported for code editing",
                        node_type.relation_str()
                    );
                    tool_call_params.tool_call_failed(err);
                    return;
                }
                // TODO: Clean up the next 20 lines or so
                let p = PathBuf::from(file);
                let file_was_relative = !p.is_absolute();
                let abs_path = match &tool_paths {
                    Some((primary, policy)) => {
                        match path_scoping::resolve_tool_path(p.as_path(), primary, policy) {
                            Ok(pb) => pb,
                            Err(err) => {
                                let msg = format!("invalid path: {}", err);
                                tool_call_params.tool_call_failed(msg);
                                return;
                            }
                        }
                    }
                    None => {
                        if p.is_absolute() {
                            p
                        } else {
                            std::env::current_dir()
                                .unwrap_or_else(|_| PathBuf::from("."))
                                .join(p)
                        }
                    }
                };
                let canon_trim = canon.trim();
                let semantic_target = match split_canon_for_semantic_target(canon_trim, *node_type)
                {
                    Ok(target) => target,
                    Err(msg) => {
                        tool_call_params.tool_call_failed(msg);
                        return;
                    }
                };
                let mod_path_owned = semantic_target.module_path().to_vec();
                let item_name = semantic_target.item_name();
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
                        if matches!(node_type, NodeType::Function) {
                            let method_target =
                                match split_canon_for_semantic_target(canon_trim, NodeType::Method)
                                {
                                    Ok(target) => target,
                                    Err(e) => {
                                        tool_call_params.tool_call_failed(e);
                                        return;
                                    }
                                };
                            let method_nodes = match ploke_db::helpers::graph_resolve_exact(
                                &state.db,
                                NodeType::Method.relation_str(),
                                &abs_path,
                                method_target.module_path(),
                                method_target.item_name(),
                            ) {
                                Ok(v) => v,
                                Err(e) => {
                                    let err = format!("DB method probe failed: {}", e);
                                    tool_call_params.tool_call_failed(err);
                                    return;
                                }
                            };
                            if method_nodes.len() == 1 {
                                let err = function_to_method_hint(
                                    name,
                                    canon,
                                    &abs_path,
                                    method_target.module_path(),
                                    method_target.owner_name(),
                                    method_target.item_name(),
                                );
                                tool_call_params.tool_call_failed_error(err);
                                return;
                            } else if method_nodes.len() > 1 {
                                let err = ambiguous_method_target_error(
                                    name,
                                    canon,
                                    &abs_path,
                                    method_target.module_path(),
                                    method_target.owner_name(),
                                    method_target.item_name(),
                                    method_nodes.len(),
                                );
                                tool_call_params.tool_call_failed_error(err);
                                return;
                            }
                        }
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
                    let err = ambiguous_method_target_error(
                        name,
                        canon,
                        &abs_path,
                        semantic_target.module_path(),
                        semantic_target.owner_name(),
                        semantic_target.item_name(),
                        nodes.len(),
                    );
                    tool_call_params.tool_call_failed_error(err);
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
                edits.push(ws);
            }
            Edit::Patch { .. } => {
                tracing::trace!("Patch found in apply_code_edit_tool call");
            }
        }
    }

    stage_semantic_edit_proposal(tool_call_params, edits).await;
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
    let editing_cfg = { state.config.read().await.editing.clone() };
    let edits: Vec<WriteSnippetData> = Vec::with_capacity(typed_req.edits.len());
    let mut patches: Vec<(String, String, String)> = typed_req
        .edits
        .into_iter()
        .filter_map(|ed| match ed {
            Edit::Patch {
                file,
                diff,
                reasoning,
            } => Some((file, diff, reasoning)),
            _ => None,
        })
        .collect();
    if patches.is_empty() {
        tool_call_params.tool_call_failed("No patches provided".to_string());
        return Ok(());
    }

    let (primary_root, policy) = state
        .with_system_read(|sys| {
            sys.tool_path_context()
                .map(|(p, pol)| (p.clone(), pol.clone()))
        })
        .await
        .ok_or_else(|| {
            ploke_error::Error::Domain(DomainError::Ui {
                message:
                    "No workspace is loaded; load a workspace before using non_semantic_patch."
                        .to_string(),
            })
        })?;

    let mut per_file: Vec<BeforeAfter> = Vec::with_capacity(patches.len());
    let mut unified_diff = String::new();
    let mut edits_ns: Vec<NsWriteSnippetData> = Vec::with_capacity(patches.len());
    let mut files: Vec<PathBuf> = Vec::with_capacity(patches.len());
    let mut display_files: Vec<String> = Vec::with_capacity(patches.len());
    let mut chat_preview_sections: Vec<String> = Vec::with_capacity(patches.len());
    let mut seen_files: BTreeSet<PathBuf> = BTreeSet::new();

    for (file, diff, _reasoning) in patches.drain(..) {
        use mpatch::ApplyOptions;
        let state_cfg = state.config.read().await;
        let apply_options = ApplyOptions::from(state_cfg.editing.patch_cfg);

        let requested_path = PathBuf::from(file.as_str());
        let abs_path = path_scoping::resolve_tool_path(
            requested_path.as_path(),
            &primary_root,
            &policy,
        )
        .map_err(|err| {
            ploke_error::Error::Domain(DomainError::Io {
                message: format!(
                    "invalid path: {err}. Paths must be absolute or workspace-root-relative."
                ),
            })
        })?;
        if !seen_files.insert(abs_path.clone()) {
            let msg = format!(
                "multiple non_semantic_patch entries targeted '{}'; combine them into one unified diff per file",
                abs_path.display()
            );
            tool_call_params.tool_call_failed(msg.clone());
            return Err(ploke_error::Error::Domain(DomainError::Io { message: msg }));
        }

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

        tracing::info!(
            request_id = %request_id,
            call_id = %call_id,
            file = %abs_path.display(),
            "ns_patch: before apply_patch_to_content"
        );
        let apply_patch_result =
            mpatch::apply_patch_to_content(&patch, Some(&content), &apply_options);
        tracing::info!(
            request_id = %request_id,
            call_id = %call_id,
            file = %abs_path.display(),
            "ns_patch: after apply_patch_to_content"
        );
        let display_path = abs_path
            .strip_prefix(&primary_root)
            .unwrap_or(abs_path.as_path())
            .to_path_buf();
        let before_after = BeforeAfter {
            file_path: display_path.clone(),
            before: content,
            after: apply_patch_result.new_content,
        };
        let options = editing_cfg.patch_cfg;
        let large_file_policy = editing_cfg.large_file_policy;
        let file_path_for_registry = file_path.clone();
        let sn_write_data = NsWriteSnippetData {
            id: request_id,
            file_path,
            expected_file_hash: file_hash,
            namespace,
            diff: Diff::from(diff),
            options,
            large_file_policy,
        };
        if matches!(editing_cfg.preview_mode, PreviewMode::Diff) {
            unified_diff.push_str(sn_write_data.diff.as_ref());
            if !unified_diff.ends_with('\n') {
                unified_diff.push('\n');
            }
        }
        chat_preview_sections.push(diff_chunk_with_context(
            &display_path,
            &before_after.before,
            &before_after.after,
            CHAT_PREVIEW_CONTEXT_LINES,
        ));
        edits_ns.push(sn_write_data);
        files.push(file_path_for_registry);
        display_files.push(display_path.display().to_string());
        per_file.push(before_after);
    }

    let preview_label = if matches!(editing_cfg.preview_mode, PreviewMode::Diff) {
        "diff"
    } else {
        "codeblock"
    };
    let max_lines = editing_cfg.max_preview_lines;
    let chat_preview_snippet = if matches!(editing_cfg.preview_mode, PreviewMode::Diff) {
        let filtered = filter_unified_diff_with_context(&unified_diff, CHAT_PREVIEW_CONTEXT_LINES);
        truncate_lines(&filtered, max_lines)
    } else {
        truncate_lines(&chat_preview_sections.join("\n"), max_lines)
    };

    tracing::info!(
        request_id = %request_id,
        call_id = %call_id,
        file_count = files.len(),
        "ns_patch: before proposals.write"
    );
    let mut reg = state.proposals.write().await;
    tracing::info!(
        request_id = %request_id,
        call_id = %call_id,
        file_count = files.len(),
        "ns_patch: after proposals.write"
    );
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
                    per_file: per_file.clone(),
                }
            },
            status: EditProposalStatus::Pending,
            is_semantic: false,
        },
    );
    drop(reg);

    tracing::info!(
        request_id = %request_id,
        call_id = %call_id,
        file_count = files.len(),
        "ns_patch: before save_proposals"
    );
    crate::app_state::handlers::proposals::save_proposals(&state).await;
    tracing::info!(
        request_id = %request_id,
        call_id = %call_id,
        file_count = files.len(),
        "ns_patch: after save_proposals"
    );

    let summary = format!(
        r#"Staged code edits (request_id: {request_id}, call_id: {call_id:?}).
Files:
    {files}

Preview (mode={preview_label}, context={context_lines} lines, first {max_lines} lines):
{preview_snippet}

Approve:  edit approve {request_id}
Deny:     edit deny {request_id}{auto_confirm}"#,
        files = display_files.join("\n  "),
        preview_label = preview_label,
        context_lines = CHAT_PREVIEW_CONTEXT_LINES,
        max_lines = max_lines,
        preview_snippet = chat_preview_snippet,
        auto_confirm = if editing_cfg.auto_confirm_edits {
            "\n\nAuto-approval enabled: applying now..."
        } else {
            ""
        },
    );
    tracing::info!(
        request_id = %request_id,
        call_id = %call_id,
        file_count = files.len(),
        "ns_patch: before add_msg_immediate_sysinfo_unpinned"
    );
    chat::add_msg_immediate_sysinfo_unpinned(&state, &event_bus, Uuid::new_v4(), summary).await;
    tracing::info!(
        request_id = %request_id,
        call_id = %call_id,
        file_count = files.len(),
        "ns_patch: after add_msg_immediate_sysinfo_unpinned"
    );
    tracing::info!(
        request_id = %request_id,
        call_id = %call_id,
        "ns_patch: apply_ns_code_edit_tool returning"
    );
    Ok(())
}
