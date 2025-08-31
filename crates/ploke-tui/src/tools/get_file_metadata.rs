use super::*;
use crate::{AppEvent, EventBus, system::SystemEvent};
use ploke_core::rag_types::GetFileMetadataResult;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

/// GetFileMetadata tool: fetches file existence, size, modified time, and tracking hash.
pub struct GetFileMetadataTool {
    pub state: Arc<crate::app_state::AppState>,
    pub event_bus: Arc<EventBus>,
    /// Parent message id for correlation in the conversation tree.
    pub parent_id: Uuid,
}

impl super::ToolFromParams for GetFileMetadataTool {
    fn build(params: &crate::rag::utils::ToolCallParams) -> Self {
        Self {
            state: Arc::clone(&params.state),
            event_bus: Arc::clone(&params.event_bus),
            parent_id: params.parent_id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct GetFileMetadataInput {
    pub file_path: String,
}

lazy_static::lazy_static! {
    static ref GET_FILE_METADATA_PARAMETERS: serde_json::Value = json!({
        "type": "object",
        "properties": {
            "file_path": {
                "type": "string",
                "description": "Absolute path to the target file."
            }
        },
        "required": ["file_path"],
        "additionalProperties": false
    });
}

impl Tool for GetFileMetadataTool {
    const NAME: &'static str = "get_file_metadata";
    const DESCRIPTION: &'static str = "Fetch current file metadata to obtain the expected_file_hash (tracking hash UUID) for safe edits.";

    type Params = GetFileMetadataInput;
    type Output = GetFileMetadataResult;

    async fn run(self, p: Self::Params) -> Result<Self::Output, ploke_error::Error> {
        use crate::rag::tools::get_file_metadata_tool;
        use crate::rag::utils::ToolCallParams;

        // Subscribe before dispatching to avoid missing fast responses
        let mut rx = self.event_bus.realtime_tx.subscribe();

        let request_id = Uuid::new_v4();
        let call_id = Uuid::new_v4().to_string();
        let args = serde_json::json!({ "file_path": p.file_path });
        let params = ToolCallParams {
            state: Arc::clone(&self.state),
            event_bus: Arc::clone(&self.event_bus),
            request_id,
            parent_id: self.parent_id,
            name: Self::NAME.to_string(),
            arguments: args,
            call_id: call_id.clone(),
        };
        get_file_metadata_tool(params).await;

        let wait = async {
            loop {
                match rx.recv().await {
                    Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                        request_id: rid,
                        call_id: cid,
                        content,
                        ..
                    })) if rid == request_id && cid == call_id => {
                        break Ok(content);
                    }
                    Ok(AppEvent::System(SystemEvent::ToolCallFailed {
                        request_id: rid,
                        call_id: cid,
                        error,
                        ..
                    })) if rid == request_id && cid == call_id => {
                        break Err(error);
                    }
                    Ok(_) => { /* ignore unrelated */ }
                    Err(e) => break Err(format!("Event channel error: {}", e)),
                }
            }
        };

        match tokio::time::timeout(std::time::Duration::from_secs(5), wait).await {
            Ok(Ok(content)) => match serde_json::from_str::<GetFileMetadataResult>(&content) {
                Ok(res) => Ok(res),
                Err(e) => Err(ploke_error::Error::Internal(
                    ploke_error::InternalError::CompilerError(format!(
                        "Failed to deserialize GetFileMetadataResult: {}",
                        e
                    )),
                )),
            },
            Ok(Err(err)) => Err(ploke_error::Error::Internal(
                ploke_error::InternalError::CompilerError(err),
            )),
            Err(_) => Err(ploke_error::Error::Internal(
                ploke_error::InternalError::CompilerError(
                    "Timed out waiting for get_file_metadata result".to_string(),
                ),
            )),
        }
    }

    fn schema() -> &'static serde_json::Value {
        GET_FILE_METADATA_PARAMETERS.deref()
    }

    fn tool_def() -> ToolFunctionDef {
        ToolFunctionDef {
            name: ToolName::GetFileMetadata,
            description: ToolDescr::GetFileMetadata,
            parameters: <GetFileMetadataTool as super::Tool>::schema().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn tool_def_serializes_expected_shape() {
        let def = <GetFileMetadataTool as Tool>::tool_def();
        let v = serde_json::to_value(&def).expect("serialize");
        let func = v.as_object().expect("def obj");
        assert_eq!(
            func.get("name").and_then(|n| n.as_str()),
            Some("get_file_metadata")
        );
        let params = func
            .get("parameters")
            .and_then(|p| p.as_object())
            .expect("params obj");
        let req = params
            .get("required")
            .and_then(|r| r.as_array())
            .expect("req arr");
        assert!(req.iter().any(|s| s.as_str() == Some("file_path")));
    }

    #[tokio::test]
    async fn run_happy_path_tempfile() {
        use std::fs;

        let state = Arc::new(crate::test_utils::mock::create_mock_app_state());
        let event_bus = Arc::new(crate::EventBus::new(
            crate::event_bus::EventBusCaps::default(),
        ));

        let tmp = std::env::temp_dir().join(format!("get_meta_{}.txt", Uuid::new_v4()));
        fs::write(&tmp, b"hello").expect("write tmp");

        let tool = GetFileMetadataTool {
            state,
            event_bus,
            parent_id: Uuid::new_v4(),
        };
        let res = super::Tool::run(
            tool,
            GetFileMetadataInput {
                file_path: tmp.display().to_string(),
            },
        )
        .await
        .expect("run ok");
        assert!(res.ok);
        assert!(res.exists);
        assert_eq!(res.byte_len, 5);
        assert_eq!(res.file_path, tmp.display().to_string());
        assert_eq!(res.tracking_hash.len(), 36);
    }
}

use std::borrow::Cow;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct GetFileMetadataParams<'a> {
    #[serde(borrow)]
    pub file_path: Cow<'a, str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GetFileMetadataParamsOwned {
    pub file_path: String,
}

impl super::GatTool for GetFileMetadataTool {
    type Output = GetFileMetadataResult;
    type OwnedParams = GetFileMetadataParamsOwned;
    type Params<'de> = GetFileMetadataParams<'de>;

    fn name() -> ToolName {
        ToolName::GetFileMetadata
    }
    fn description() -> ToolDescr {
        ToolDescr::GetFileMetadata
    }
    fn schema() -> &'static serde_json::Value {
        &GET_FILE_METADATA_PARAMETERS
    }

    fn build(ctx: &super::Ctx) -> Self {
        Self {
            state: Arc::clone(&ctx.state),
            event_bus: Arc::clone(&ctx.event_bus),
            parent_id: ctx.parent_id,
        }
    }

    fn into_owned<'a>(params: &Self::Params<'a>) -> Self::OwnedParams {
        GetFileMetadataParamsOwned {
            file_path: params.file_path.clone().into_owned(),
        }
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        use ploke_core::PROJECT_NAMESPACE_UUID;
        let path = PathBuf::from(params.file_path.as_ref());
        match tokio::fs::read(&path).await {
            Ok(bytes) => {
                let hash_uuid = uuid::Uuid::new_v5(&PROJECT_NAMESPACE_UUID, &bytes);
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
                let file_meta = GetFileMetadataResult {
                    ok: true,
                    file_path: path.display().to_string(),
                    exists: true,
                    byte_len,
                    modified_ms,
                    file_hash: hash_uuid.to_string(),
                    tracking_hash: hash_uuid.to_string(),
                };

                let serialized_str =
                    serde_json::to_string(&file_meta).expect("Incorrect deserialization");
                Ok(ToolResult {
                    content: serialized_str,
                })
            }
            Err(e) => Err(ploke_error::Error::Internal(
                ploke_error::InternalError::CompilerError(format!(
                    "Failed to read file '{}': {}",
                    path.display(),
                    e
                )),
            )),
        }
    }
}

#[cfg(test)]
mod gat_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn params_deserialize_and_into_owned() {
        let raw = r#"{"file_path":"/tmp/somefile.txt"}"#;
        let params = GetFileMetadataTool::deserialize_params(raw).expect("parse");
        assert_eq!(params.file_path.as_ref(), "/tmp/somefile.txt");
        let owned = GetFileMetadataTool::into_owned(&params);
        assert_eq!(owned.file_path, "/tmp/somefile.txt");
    }

    #[test]
    fn name_desc_and_schema_present() {
        assert!(matches!(GetFileMetadataTool::name(), ToolName::GetFileMetadata));
        assert!(matches!(GetFileMetadataTool::description(), ToolDescr::GetFileMetadata));
        let schema = <GetFileMetadataTool as super::GatTool>::schema();
        let props = schema
            .as_object()
            .and_then(|o| o.get("properties"))
            .and_then(|p| p.as_object())
            .expect("props obj");
        assert!(props.contains_key("file_path"));
    }

    #[tokio::test]
    async fn execute_happy_path() {
        use crate::event_bus::EventBusCaps;
        let state = Arc::new(crate::test_utils::mock::create_mock_app_state());
        let event_bus = Arc::new(crate::EventBus::new(EventBusCaps::default()));
        let ctx = Ctx {
            state,
            event_bus,
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            call_id: Arc::<str>::from("get-meta-test"),
        };

        let tmp = std::env::temp_dir().join(format!("get_meta_gat_{}.txt", Uuid::new_v4()));
        std::fs::write(&tmp, b"hello").expect("write tmp");

        let params = GetFileMetadataParams { file_path: Cow::Owned(tmp.display().to_string()) };
        let out = GetFileMetadataTool::execute(params, ctx).await.expect("ok");
        let parsed: GetFileMetadataResult = serde_json::from_str(&out.content).expect("json");
        assert!(parsed.ok);
        assert!(parsed.exists);
        assert_eq!(parsed.byte_len, 5);
        assert_eq!(parsed.file_path, tmp.display().to_string());
        assert_eq!(parsed.tracking_hash.len(), 36);
    }

    #[tokio::test]
    async fn execute_missing_file_errors() {
        use crate::event_bus::EventBusCaps;
        let state = Arc::new(crate::test_utils::mock::create_mock_app_state());
        let event_bus = Arc::new(crate::EventBus::new(EventBusCaps::default()));
        let ctx = Ctx {
            state,
            event_bus,
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            call_id: Arc::<str>::from("get-meta-test-missing"),
        };
        let params = GetFileMetadataParams { file_path: Cow::Borrowed("/no/such/file/and/path.txt") };
        let out = GetFileMetadataTool::execute(params, ctx).await;
        assert!(out.is_err());
    }
}
