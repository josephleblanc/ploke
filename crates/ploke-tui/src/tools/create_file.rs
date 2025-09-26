use super::*;
use crate::EventBus;
use ploke_core::rag_types::CreateFileResult;

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

impl super::Tool for CreateFile {
    type Output = CreateFileResult;
    type OwnedParams = CreateFileParamsOwned;
    type Params<'de> = CreateFileParams<'de>;

    fn name() -> super::ToolName { super::ToolName::CreateFile }
    fn description() -> super::ToolDescr { super::ToolDescr::CreateFile }
    fn schema() -> &'static serde_json::Value { CREATE_FILE_PARAMETERS.deref() }

    fn build(_ctx: &super::Ctx) -> Self { Self }

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
        use crate::rag::tools::create_file_tool;
        use crate::rag::utils::ToolCallParams;

        let arguments = serde_json::to_value(CreateFileParamsOwned {
            file_path: params.file_path.clone().into_owned(),
            content: params.content.clone().into_owned(),
            on_exists: params.on_exists.as_ref().map(|s| s.clone().into_owned()),
            create_parents: params.create_parents,
        })
        .map_err(|e| ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(format!("Failed to serialize CreateFileParams: {}", e))))?;

        let env = ToolCallParams {
            state: Arc::clone(&ctx.state),
            event_bus: Arc::clone(&ctx.event_bus),
            request_id: ctx.request_id,
            parent_id: ctx.parent_id,
            name: Self::name(),
            arguments,
            call_id: ctx.call_id.clone(),
        };
        create_file_tool(env).await;

        // Build typed result deterministically from proposal registry
        let proposal_opt = { ctx.state.create_proposals.read().await.get(&ctx.request_id).cloned() };
        if let Some(prop) = proposal_opt {
            let crate_root = { ctx.state.system.read().await.crate_focus.clone() };
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
            let s = serde_json::to_string(&structured).expect("serialize result");
            return Ok(ToolResult { content: s });
        }

        Err(ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(
            "create_file failed to stage proposal (see ToolCallFailed)".to_string(),
        )))
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

