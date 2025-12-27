use super::*;
use crate::{
    EventBus,
    app_state::{
        core::{BeforeAfter, CreateProposal, EditProposalStatus, PreviewMode},
        handlers::chat,
    },
    chat_history::MessageKind,
    rag::editing,
};
use ploke_core::rag_types::CreateFileResult;
use similar::TextDiff;

pub struct CreateFile;

#[derive(Debug, Clone, Deserialize)]
pub struct CreateFileParams<'a> {
    #[serde(borrow)]
    pub file_path: std::borrow::Cow<'a, str>,
    #[serde(borrow)]
    pub content: std::borrow::Cow<'a, str>,
    #[serde(default)]
    pub on_exists: Option<std::borrow::Cow<'a, str>>, // "error" | "overwrite"
    #[serde(default)]
    pub create_parents: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateFileParamsOwned {
    pub file_path: String,
    pub content: String,
    pub on_exists: Option<String>,
    pub create_parents: bool,
}

#[derive(Clone, Debug)]
pub struct CreateFileCtx {
    pub state: Arc<AppState>,
    pub event_bus: Arc<EventBus>,
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub name: ToolName,
    pub typed_req: CreateFileParamsOwned,
    pub call_id: ArcStr,
}

impl super::Tool for CreateFile {
    type Output = CreateFileResult;
    type OwnedParams = CreateFileParamsOwned;
    type Params<'de> = CreateFileParams<'de>;

    fn name() -> super::ToolName {
        super::ToolName::CreateFile
    }
    fn description() -> super::ToolDescr {
        super::ToolDescr::CreateFile
    }
    fn schema() -> &'static serde_json::Value {
        CREATE_FILE_PARAMETERS.deref()
    }

    fn build(_ctx: &super::Ctx) -> Self {
        Self
    }

    fn into_owned<'a>(params: &Self::Params<'a>) -> Self::OwnedParams {
        CreateFileParamsOwned {
            file_path: params.file_path.clone().into_owned(),
            content: params.content.clone().into_owned(),
            on_exists: params.on_exists.as_ref().map(|s| s.clone().into_owned()),
            create_parents: params.create_parents,
        }
    }

    async fn execute<'a>(
        params: Self::Params<'a>,
        ctx: super::Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        use crate::rag::utils::ToolCallParams;

        let typed_req = CreateFileParamsOwned {
            file_path: params.file_path.clone().into_owned(),
            content: params.content.clone().into_owned(),
            on_exists: params.on_exists.as_ref().map(|s| s.clone().into_owned()),
            create_parents: params.create_parents,
        };

        let create_file_ctx = CreateFileCtx {
            state: Arc::clone(&ctx.state),
            event_bus: Arc::clone(&ctx.event_bus),
            request_id: ctx.request_id,
            parent_id: ctx.parent_id,
            name: Self::name(),
            typed_req,
            call_id: ctx.call_id.clone(),
        };
        create_file_tool(create_file_ctx).await;

        // Build typed result deterministically from proposal registry
        let proposal_opt = {
            ctx.state
                .create_proposals
                .read()
                .await
                .get(&ctx.request_id)
                .cloned()
        };
        if let Some(prop) = proposal_opt {
            let crate_root = { ctx.state.system.read().await.focused_crate_root() };
            tracing::debug!(crate_root = ?crate_root);
            let display_files: Vec<String> = prop
                .files
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
            let editing_cfg = { ctx.state.config.read().await.editing.clone() };
            let preview_mode = match editing_cfg.preview_mode {
                crate::app_state::core::PreviewMode::Diff => "diff".to_string(),
                crate::app_state::core::PreviewMode::CodeBlock => "codeblock".to_string(),
            };
            let structured = CreateFileResult {
                ok: true,
                staged: prop.creates.len(),
                applied: 0,
                files: display_files,
                preview_mode,
                auto_confirmed: editing_cfg.auto_confirm_edits,
            };
            let summary = format!("Staged {} files for creation", structured.staged);
            let ui_payload = crate::tools::ToolUiPayload::new(
                ToolName::CreateFile,
                ctx.call_id.clone(),
                summary,
            )
            .with_field("staged", structured.staged.to_string())
            .with_field("applied", structured.applied.to_string())
            .with_field("files", structured.files.len().to_string())
            .with_field("preview_mode", structured.preview_mode.as_str());
            let s = serde_json::to_string(&structured).expect("serialize result");
            return Ok(ToolResult {
                content: s,
                ui_payload: Some(ui_payload),
            });
        }

        Err(ploke_error::Error::Internal(
            ploke_error::InternalError::CompilerError(
                "create_file failed to stage proposal (see ToolCallFailed)".to_string(),
            ),
        ))
    }
}

impl CreateFileCtx {
    fn tool_error_from_message(&self, message: impl Into<String>) -> ToolError {
        ToolError::new(self.name, ToolErrorCode::InvalidFormat, message)
    }

    pub(super) fn tool_call_failed(&self, error: String) {
        let err = self.tool_error_from_message(error);
        self.tool_call_failed_error(err);
    }

    pub(super) fn tool_call_failed_error(&self, error: ToolError) {
        let _ = self
            .event_bus
            .realtime_tx
            .send(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id: self.request_id,
                parent_id: self.parent_id,
                call_id: self.call_id.clone(),
                error: error.to_wire_string(),
                ui_payload: Some(ToolUiPayload::from_error(self.call_id.clone(), &error)),
            }));
    }
    pub(super) fn tool_call_err(&self, error: String) -> SystemEvent {
        let err = self.tool_error_from_message(error);
        self.tool_call_err_from_error(err)
    }

    pub(super) fn tool_call_err_from_error(&self, error: ToolError) -> SystemEvent {
        SystemEvent::ToolCallFailed {
            request_id: self.request_id,
            parent_id: self.parent_id,
            call_id: self.call_id.clone(),
            error: error.to_wire_string(),
            ui_payload: Some(ToolUiPayload::from_error(self.call_id.clone(), &error)),
        }
    }
}

pub async fn create_file_tool(tool_call_params: CreateFileCtx) {
    use ploke_core::{CreateFileData, OnExists, PROJECT_NAMESPACE_UUID};
    let CreateFileCtx {
        state,
        event_bus,
        request_id,
        parent_id,
        name,
        typed_req,
        call_id,
    } = tool_call_params.clone();

    let params: CreateFileParamsOwned = typed_req.clone();

    // Resolve absolute path against crate root when relative
    let crate_root = { state.system.read().await.focused_crate_root() };
    let abs_path = {
        let p = std::path::PathBuf::from(&params.file_path);
        if let Some(root) = crate_root.as_ref() {
            match crate::utils::path_scoping::resolve_in_crate_root(&p, root) {
                Ok(pb) => pb,
                Err(err) => {
                    tool_call_params.tool_call_failed(format!("invalid path: {}", err));
                    return;
                }
            }
        } else if p.is_absolute() {
            p
        } else {
            tracing::warn!("No crate focus set, falling back to pwd");
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join(p)
        }
    };
    tracing::debug!(crate_root = ?crate_root, abs_path = ?abs_path);

    // Restrict to .rs files
    if abs_path.extension().and_then(|e| e.to_str()) != Some("rs") {
        tool_call_params.tool_call_failed("only .rs files are supported".to_string());
        return;
    }

    let on_exists = match params.on_exists.as_deref() {
        Some("overwrite") => OnExists::Overwrite,
        Some("error") | None => OnExists::Error,
        Some(other) => {
            tool_call_params.tool_call_failed(format!("invalid on_exists: {}", other));
            return;
        }
    };

    // Idempotency: prevent duplicate staging for same request_id
    {
        let reg = state.create_proposals.read().await;
        if reg.contains_key(&request_id) {
            let msg = format!(
                "Duplicate create_file request ignored for request_id {}",
                request_id
            );
            tool_call_params.tool_call_failed(msg.clone());
            chat::add_msg_immediate(
                &state,
                &event_bus,
                uuid::Uuid::new_v4(),
                msg,
                MessageKind::SysInfo,
            )
            .await;
            return;
        }
    }

    // Build IO request
    let create_req = CreateFileData {
        id: uuid::Uuid::new_v4(),
        name: abs_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| abs_path.display().to_string()),
        file_path: abs_path.clone(),
        content: params.content,
        namespace: PROJECT_NAMESPACE_UUID,
        on_exists,
        create_parents: params.create_parents,
    };

    // Preview generation
    let editing_cfg = { state.config.read().await.editing.clone() };
    let before = String::new();
    let after = create_req.content.clone();
    let display_path = if let Some(root) = crate_root.as_ref() {
        abs_path
            .strip_prefix(root)
            .unwrap_or(abs_path.as_path())
            .to_path_buf()
    } else {
        abs_path.clone()
    };

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

    let per_file = vec![BeforeAfter {
        file_path: display_path.clone(),
        before: truncate(&before),
        after: truncate(&after),
    }];

    let mut unified_diff = String::new();
    if matches!(editing_cfg.preview_mode, PreviewMode::Diff) {
        let header_a = "/dev/null".to_string();
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

    // Stash proposal
    {
        let mut reg = state.create_proposals.write().await;
        reg.insert(
            request_id,
            CreateProposal {
                request_id,
                parent_id,
                call_id: call_id.clone(),
                proposed_at_ms: chrono::Utc::now().timestamp_millis(),
                creates: vec![create_req.clone()],
                files: vec![abs_path.clone()],
                preview: if matches!(editing_cfg.preview_mode, PreviewMode::Diff) {
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
    crate::app_state::handlers::proposals::save_create_proposals(&state).await;

    // Emit SysInfo summary
    let preview_label = if matches!(editing_cfg.preview_mode, PreviewMode::Diff) {
        "diff"
    } else {
        "codeblock"
    };
    let summary = format!(
        r#"Staged file creation (request_id: {request_id}, call_id: {call_id:?}).
Files:
    {file}

Preview (mode={preview}, first {lines} lines per section):
{snippet}

Approve:  create approve {request_id}
Deny:     create deny {request_id}{auto}"#,
        file = display_path.display(),
        preview = preview_label,
        lines = editing_cfg.max_preview_lines,
        snippet = if matches!(editing_cfg.preview_mode, PreviewMode::Diff) {
            unified_diff.clone()
        } else {
            format!("Before:\n\nAfter:\n{}", per_file[0].after)
        },
        auto = if editing_cfg.auto_confirm_edits {
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

    // Emit typed ToolCallCompleted result
    let result = ploke_core::rag_types::CreateFileResult {
        ok: true,
        staged: 1,
        applied: 0,
        files: vec![display_path.display().to_string()],
        preview_mode: preview_label.to_string(),
        auto_confirmed: editing_cfg.auto_confirm_edits,
    };
    let ui_payload = ToolUiPayload::new(
        ToolName::CreateFile,
        call_id.clone(),
        format!("Staged {} files for creation", result.staged),
    )
    .with_field("staged", result.staged.to_string())
    .with_field("applied", result.applied.to_string())
    .with_field("files", result.files.len().to_string())
    .with_field("preview_mode", result.preview_mode.as_str())
    .with_field("auto_confirmed", result.auto_confirmed.to_string());
    let content = match serde_json::to_string(&result) {
        Ok(s) => s,
        Err(e) => {
            tool_call_params
                .tool_call_failed(format!("Failed to serialize CreateFileResult: {}", e));
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
            editing::approve_creations(&state2, &event_bus2, request_id).await;
        });
    }
}

lazy_static::lazy_static! {
    static ref CREATE_FILE_PARAMETERS: serde_json::Value = json!({
        "type": "object",
        "properties": {
            "file_path": { "type": "string", "description": "Absolute or workspace-relative path to new Rust file (.rs)." },
            "content": { "type": "string", "description": "Full file content." },
            "on_exists": { "type": "string", "enum": ["error", "overwrite"], "description": "Policy when file already exists (default: error)." },
            "create_parents": { "type": "boolean", "description": "Create parent directories if missing (default: false)." }
        },
        "required": ["file_path", "content"],
        "additionalProperties": false
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_contains_expected_fields() {
        let schema = CreateFile::schema();
        let obj = schema.as_object().expect("obj");
        assert!(obj.contains_key("properties"));
        let props = obj.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("file_path"));
        assert!(props.contains_key("content"));
        assert!(props.contains_key("on_exists"));
        assert!(props.contains_key("create_parents"));
    }

    #[test]
    fn deserialize_and_into_owned() {
        let raw = r#"{ "file_path": "src/new.rs", "content": "fn main(){}", "on_exists": "error", "create_parents": true }"#;
        let params = CreateFile::deserialize_params(raw).expect("parse");
        let owned = CreateFile::into_owned(&params);
        assert_eq!(owned.file_path, "src/new.rs");
        assert_eq!(owned.content, "fn main(){}");
        assert_eq!(owned.on_exists.as_deref(), Some("error"));
        assert!(owned.create_parents);
    }
}
