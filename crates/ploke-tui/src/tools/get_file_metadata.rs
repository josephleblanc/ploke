use super::*;
use crate::{AppEvent, EventBus, system::SystemEvent};
use ploke_core::rag_types::GetFileMetadataResult;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

/// GetFileMetadata tool: fetches file existence, size, modified time, and tracking hash.
pub struct GetFileMetadata {
    pub state: Arc<crate::app_state::AppState>,
    pub event_bus: Arc<EventBus>,
    /// Parent message id for correlation in the conversation tree.
    pub parent_id: Uuid,
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
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn tool_def_serializes_expected_shape() -> color_eyre::Result<()> {
        let def = <GetFileMetadata as Tool>::tool_def();
        let v = serde_json::to_value(&def).expect("serialize");
        eprintln!("{}", serde_json::to_string_pretty(&v)?);
        let func = v.as_object().expect("def obj");
        // Tool definition should have the correct OpenRouter structure
        assert_eq!(
            func.get("type").and_then(|t| t.as_str()),
            Some("function")
        );
        let function = func.get("function").and_then(|f| f.as_object()).expect("function obj");
        assert_eq!(
            function.get("name").and_then(|n| n.as_str()),
            Some("get_file_metadata")
        );
        let params = function
            .get("parameters")
            .and_then(|p| p.as_object())
            .expect("params obj");
        let req = params
            .get("required")
            .and_then(|r| r.as_array())
            .expect("req arr");
        assert!(req.iter().any(|s| s.as_str() == Some("file_path")));
        Ok(())
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

impl super::Tool for GetFileMetadata {
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
        let params = GetFileMetadata::deserialize_params(raw).expect("parse");
        assert_eq!(params.file_path.as_ref(), "/tmp/somefile.txt");
        let owned = GetFileMetadata::into_owned(&params);
        assert_eq!(owned.file_path, "/tmp/somefile.txt");
    }

    #[test]
    fn name_desc_and_schema_present() {
        assert!(matches!(GetFileMetadata::name(), ToolName::GetFileMetadata));
        assert!(matches!(
            GetFileMetadata::description(),
            ToolDescr::GetFileMetadata
        ));
        let schema = <GetFileMetadata as super::Tool>::schema();
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

        let params = GetFileMetadataParams {
            file_path: Cow::Owned(tmp.display().to_string()),
        };
        let out = GetFileMetadata::execute(params, ctx).await.expect("ok");
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
        let params = GetFileMetadataParams {
            file_path: Cow::Borrowed("/no/such/file/and/path.txt"),
        };
        let out = GetFileMetadata::execute(params, ctx).await;
        assert!(out.is_err());
    }

    #[test]
    fn de_to_value() -> color_eyre::Result<()> {
        let def = <GetFileMetadata as Tool>::tool_def();
        let v = serde_json::to_value(&def).expect("serialize");
        eprintln!("{}", serde_json::to_string_pretty(&v)?);
        let expected = json!({
            "type": "function",
            "function": {
                "name": "get_file_metadata",
                "description": "Fetch current file metadata to obtain the expected_file_hash (tracking hash UUID) for safe edits.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "Absolute path to the target file."
                        }
                    },
                    "required": ["file_path"]
                }
            }
        });
        assert_eq!(expected, v);

        Ok(())
    }
}
