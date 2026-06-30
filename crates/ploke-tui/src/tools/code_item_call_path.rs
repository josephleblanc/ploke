use std::{borrow::Cow, ops::Deref};

use ploke_core::{
    rag_types::{CallPathInfo, NodeFilepath},
    tool_descriptions::ToolDescription,
    tool_types::ToolName,
};
use ploke_db::CallPathOptions;
use serde::{Deserialize, Serialize};

use crate::tools::{Tool, ToolError, ToolErrorCode, ToolInvocationError, lookup_support};

const ITEM_DESC: &str = "Exact Rust code item coordinate.";
const MAX_DEPTH_DESC: &str = "Maximum resolved call-path depth. Defaults to 3.";
const MAX_PATHS_DESC: &str = "Maximum number of paths to return. Defaults to 64.";

lazy_static::lazy_static! {
    static ref CODE_ITEM_CALL_PATH_PARAMETERS: serde_json::Value = serde_json::json!({
        "type": "object",
        "properties": {
            "source": { "$ref": "#/$defs/code_item_endpoint", "description": ITEM_DESC },
            "target": { "$ref": "#/$defs/code_item_endpoint", "description": ITEM_DESC },
            "max_depth": { "type": "integer", "minimum": 1, "description": MAX_DEPTH_DESC },
            "max_paths": { "type": "integer", "minimum": 1, "description": MAX_PATHS_DESC }
        },
        "required": ["source", "target"],
        "additionalProperties": false,
        "$defs": {
            "code_item_endpoint": {
                "type": "object",
                "properties": {
                    "item_name": { "type": "string" },
                    "file_path": { "type": "string" },
                    "node_kind": crate::rag::utils::NodeKind::schema_property(),
                    "module_path": {
                        "type": "string",
                        "description": lookup_support::MODULE_PATH_DESC
                    },
                    "owner_trait": {
                        "type": "string",
                        "description": lookup_support::OWNER_TRAIT_DESC
                    },
                    "owner_type": {
                        "type": "string",
                        "description": lookup_support::OWNER_TYPE_DESC
                    }
                },
                "required": ["item_name", "file_path", "node_kind", "module_path"],
                "additionalProperties": false
            }
        }
    });
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodeItemCallPathEndpoint<'a> {
    #[serde(borrow)]
    pub item_name: Cow<'a, str>,
    #[serde(borrow)]
    pub file_path: Cow<'a, str>,
    #[serde(borrow)]
    pub node_kind: Cow<'a, str>,
    #[serde(borrow)]
    pub module_path: Cow<'a, str>,
    #[serde(default, borrow)]
    pub owner_trait: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub owner_type: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodeItemCallPathParams<'a> {
    #[serde(borrow)]
    pub source: CodeItemCallPathEndpoint<'a>,
    #[serde(borrow)]
    pub target: CodeItemCallPathEndpoint<'a>,
    #[serde(default)]
    pub max_depth: Option<u32>,
    #[serde(default)]
    pub max_paths: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tool_contracts", derive(Deserialize))]
pub struct CodeItemCallPathEndpointOwned {
    pub item_name: String,
    pub file_path: String,
    pub node_kind: String,
    pub module_path: String,
    pub owner_trait: Option<String>,
    pub owner_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tool_contracts", derive(Deserialize))]
pub struct CodeItemCallPathParamsOwned {
    pub source: CodeItemCallPathEndpointOwned,
    pub target: CodeItemCallPathEndpointOwned,
    pub max_depth: Option<u32>,
    pub max_paths: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeItemCallPathResult {
    pub source_id: uuid::Uuid,
    pub target_id: uuid::Uuid,
    pub source_file_path: NodeFilepath,
    pub target_file_path: NodeFilepath,
    pub reachable: bool,
    pub max_depth: u32,
    pub max_paths: usize,
    pub paths: Vec<CallPathInfo>,
}

pub struct CodeItemCallPath;

impl Tool for CodeItemCallPath {
    type Output = CodeItemCallPathResult;

    type OwnedParams = CodeItemCallPathParamsOwned;

    type Params<'de>
        = CodeItemCallPathParams<'de>
    where
        Self: 'de;

    fn name() -> ToolName {
        ToolName::CodeItemCallPath
    }

    fn description() -> ToolDescription {
        Self::name().description()
    }

    fn schema() -> &'static serde_json::Value {
        CODE_ITEM_CALL_PATH_PARAMETERS.deref()
    }

    fn adapt_error(err: ToolInvocationError) -> ToolError {
        match err {
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Ui { message },
            )) => ToolError::new(
                ToolName::CodeItemCallPath,
                ToolErrorCode::InvalidFormat,
                message,
            )
            .retry_hint(lookup_support::LOOKUP_RETRY_HINT),
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Io { message },
            )) => ToolError::new(ToolName::CodeItemCallPath, ToolErrorCode::Io, message)
                .retry_hint(lookup_support::LOOKUP_RETRY_HINT),
            other => other.into_tool_error(ToolName::CodeItemCallPath),
        }
    }

    fn build(_ctx: &super::Ctx) -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams {
        CodeItemCallPathParamsOwned {
            source: endpoint_to_owned(&params.source),
            target: endpoint_to_owned(&params.target),
            max_depth: params.max_depth,
            max_paths: params.max_paths,
        }
    }

    fn deserialize_params<'a>(json: &'a str) -> Result<Self::Params<'a>, ToolInvocationError> {
        let params: CodeItemCallPathParams<'a> =
            serde_json::from_str(json).map_err(|source| ToolInvocationError::Deserialize {
                source,
                raw: Some(json.to_string()),
            })?;
        lookup_support::validate_module_path(Self::name(), params.source.module_path.as_ref())?;
        lookup_support::validate_module_path(Self::name(), params.target.module_path.as_ref())?;
        Ok(params)
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: super::Ctx,
    ) -> Result<super::ToolResult, ploke_error::Error> {
        use ploke_error::{DomainError, InternalError};

        ctx.state.is_stale_err().await?;
        validate_endpoint("source", &params.source)?;
        validate_endpoint("target", &params.target)?;

        let max_depth = params.max_depth.unwrap_or(3);
        let max_paths = params.max_paths.unwrap_or(64);
        if max_depth == 0 || max_paths == 0 {
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: "max_depth and max_paths must be greater than zero.".to_string(),
            }));
        }

        let (primary_root, policy) = ctx
            .state
            .with_system_read(|sys| {
                sys.tool_path_context()
                    .map(|(root, policy)| (root.clone(), policy.clone()))
            })
            .await
            .ok_or_else(|| {
                ploke_error::Error::Domain(DomainError::Ui {
                    message:
                        "No workspace is loaded; load a workspace before using code_item_call_path."
                            .to_string(),
                })
            })?;

        let source = resolve_endpoint(&ctx, &primary_root, &policy, &params.source)?;
        let target = resolve_endpoint(&ctx, &primary_root, &policy, &params.target)?;
        let paths = match ctx.state.rag.as_ref() {
            Some(rag) if !rag.call_context_degraded() => rag
                .exact_call_paths_between(
                    source.id,
                    target.id,
                    CallPathOptions {
                        max_depth,
                        max_paths,
                    },
                )
                .map_err(|err| {
                    ploke_error::Error::Internal(InternalError::CompilerError(format!(
                        "failed to collect call paths between {} and {}: {err}",
                        source.id, target.id
                    )))
                })?,
            _ => Vec::new(),
        };
        let result = CodeItemCallPathResult {
            source_id: source.id,
            target_id: target.id,
            source_file_path: NodeFilepath::new(source.rel_path.display().to_string()),
            target_file_path: NodeFilepath::new(target.rel_path.display().to_string()),
            reachable: !paths.is_empty(),
            max_depth,
            max_paths,
            paths,
        };
        let summary = if result.reachable {
            format!("Found {} call path(s)", result.paths.len())
        } else {
            "No resolved call path found".to_string()
        };
        let ui_payload = super::ToolUiPayload::new(Self::name(), ctx.call_id.clone(), summary)
            .with_field("source_id", result.source_id.to_string())
            .with_field("target_id", result.target_id.to_string())
            .with_field("reachable", result.reachable.to_string())
            .with_field("paths", result.paths.len().to_string())
            .with_field("max_depth", result.max_depth.to_string())
            .with_field("max_paths", result.max_paths.to_string());
        let content = serde_json::to_string(&result).map_err(|err| {
            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                "failed to serialize CodeItemCallPathResult: {err}. This indicates an error in the ploke application itself, not due to incorrect search terms. Please consider filing an issue on the ploke github."
            )))
        })?;

        Ok(super::ToolResult {
            content,
            ui_payload: Some(ui_payload),
        })
    }
}

fn endpoint_to_owned(endpoint: &CodeItemCallPathEndpoint<'_>) -> CodeItemCallPathEndpointOwned {
    CodeItemCallPathEndpointOwned {
        item_name: endpoint.item_name.clone().into_owned(),
        file_path: endpoint.file_path.clone().into_owned(),
        node_kind: endpoint.node_kind.clone().into_owned(),
        module_path: endpoint.module_path.clone().into_owned(),
        owner_trait: endpoint.owner_trait.as_ref().map(ToString::to_string),
        owner_type: endpoint.owner_type.as_ref().map(ToString::to_string),
    }
}

fn validate_endpoint(
    label: &str,
    endpoint: &CodeItemCallPathEndpoint<'_>,
) -> Result<(), ploke_error::Error> {
    for (field, value) in [
        ("item_name", endpoint.item_name.as_ref()),
        ("file_path", endpoint.file_path.as_ref()),
        ("node_kind", endpoint.node_kind.as_ref()),
        ("module_path", endpoint.module_path.as_ref()),
    ] {
        if value.trim().is_empty() {
            return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                message: format!("{label}.{field} must not be empty."),
            }));
        }
    }
    Ok(())
}

fn resolve_endpoint(
    ctx: &super::Ctx,
    primary_root: &std::path::Path,
    policy: &ploke_io::path_policy::PathPolicy,
    endpoint: &CodeItemCallPathEndpoint<'_>,
) -> Result<lookup_support::ResolvedToolItem, ploke_error::Error> {
    lookup_support::resolve_exact_tool_item(
        &ctx.state.db,
        primary_root,
        policy,
        lookup_support::ExactItemRequest {
            item_name: endpoint.item_name.as_ref(),
            file_path: endpoint.file_path.as_ref(),
            node_kind: endpoint.node_kind.as_ref(),
            module_path: endpoint.module_path.as_ref(),
            owner_trait: endpoint.owner_trait.as_deref(),
            owner_type: endpoint.owner_type.as_deref(),
        },
    )
}
