use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Deref,
};

use ploke_core::{
    rag_types::{CallPathInfo, NodeFilepath, ProofContextInfo},
    tool_descriptions::ToolDescription,
    tool_types::ToolName,
};
use ploke_db::CallPathOptions;
use serde::{Deserialize, Serialize};

use crate::tools::{
    Tool, ToolError, ToolErrorCode, ToolInvocationError, code_item_endpoint, lookup_support,
};
use code_item_endpoint::{
    CodeItemEndpoint, CodeItemEndpointOwned, endpoint_to_owned, resolve_endpoint, validate_endpoint,
};

const ITEM_DESC: &str = "Exact Rust code item coordinate.";
const MAX_DEPTH_DESC: &str = "Maximum resolved call-path depth. Defaults to 3.";
const MAX_PATHS_DESC: &str = "Maximum number of paths to return. Defaults to 64.";

lazy_static::lazy_static! {
    static ref CODE_ITEM_CALL_PATH_PARAMETERS: serde_json::Value = serde_json::json!({
        "type": "object",
        "properties": {
            "source": { "$ref": "#/$defs/code_item_endpoint", "description": ITEM_DESC },
            "target": { "$ref": "#/$defs/code_item_endpoint", "description": ITEM_DESC },
            "guard": {
                "$ref": "#/$defs/code_item_endpoint",
                "description": "Optional exact code item that must appear before target on every resolved path."
            },
            "max_depth": { "type": "integer", "minimum": 1, "description": MAX_DEPTH_DESC },
            "max_paths": { "type": "integer", "minimum": 1, "description": MAX_PATHS_DESC }
        },
        "required": ["source", "target"],
        "additionalProperties": false,
        "$defs": {
            "code_item_endpoint": code_item_endpoint::schema_property()
        }
    });
}

pub type CodeItemCallPathEndpoint<'a> = CodeItemEndpoint<'a>;

#[derive(Debug, Clone, Deserialize)]
pub struct CodeItemCallPathParams<'a> {
    #[serde(borrow)]
    pub source: CodeItemCallPathEndpoint<'a>,
    #[serde(borrow)]
    pub target: CodeItemCallPathEndpoint<'a>,
    #[serde(default, borrow)]
    pub guard: Option<CodeItemCallPathEndpoint<'a>>,
    #[serde(default)]
    pub max_depth: Option<u32>,
    #[serde(default)]
    pub max_paths: Option<usize>,
}

pub type CodeItemCallPathEndpointOwned = CodeItemEndpointOwned;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tool_contracts", derive(Deserialize))]
pub struct CodeItemCallPathParamsOwned {
    pub source: CodeItemCallPathEndpointOwned,
    pub target: CodeItemCallPathEndpointOwned,
    #[serde(default)]
    pub guard: Option<CodeItemCallPathEndpointOwned>,
    pub max_depth: Option<u32>,
    pub max_paths: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeItemCallPathResult {
    pub source_id: uuid::Uuid,
    pub target_id: uuid::Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard_id: Option<uuid::Uuid>,
    pub source_file_path: NodeFilepath,
    pub target_file_path: NodeFilepath,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard_file_path: Option<NodeFilepath>,
    pub reachable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guarded: Option<bool>,
    pub max_depth: u32,
    pub max_paths: usize,
    pub paths: Vec<CallPathInfo>,
    #[serde(default)]
    pub violations: Vec<CallPathInfo>,
    pub source_files: Vec<NodeFilepath>,
    pub proof_context: Vec<ProofContextInfo>,
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
            guard: params.guard.as_ref().map(endpoint_to_owned),
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
        if let Some(guard) = params.guard.as_ref() {
            lookup_support::validate_module_path(Self::name(), guard.module_path.as_ref())?;
        }
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
        if let Some(guard) = params.guard.as_ref() {
            validate_endpoint("guard", guard)?;
        }

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
        let guard = params
            .guard
            .as_ref()
            .map(|endpoint| resolve_endpoint(&ctx, &primary_root, &policy, endpoint))
            .transpose()?;
        let options = CallPathOptions {
            max_depth,
            max_paths,
        };
        let (paths, guarded, violations) = match (ctx.state.rag.as_ref(), guard.as_ref()) {
            (Some(rag), Some(guard)) if !rag.call_context_degraded() => {
                let report = rag
                    .exact_call_guard_report_between(source.id, target.id, guard.id, options)
                    .map_err(|err| {
                        ploke_error::Error::Internal(InternalError::CompilerError(format!(
                            "failed to collect guarded call paths between {} and {} through {}: {err}",
                            source.id, target.id, guard.id
                        )))
                    })?;
                match report {
                    Some(report) => (report.paths, Some(report.guarded), report.violations),
                    None => (Vec::new(), Some(false), Vec::new()),
                }
            }
            (Some(rag), None) if !rag.call_context_degraded() => (
                rag.exact_call_paths_between(source.id, target.id, options)
                    .map_err(|err| {
                        ploke_error::Error::Internal(InternalError::CompilerError(format!(
                            "failed to collect call paths between {} and {}: {err}",
                            source.id, target.id
                        )))
                    })?,
                None,
                Vec::new(),
            ),
            (_, Some(_)) => (Vec::new(), Some(false), Vec::new()),
            _ => (Vec::new(), None, Vec::new()),
        };
        let guard_id = guard.as_ref().map(|guard| guard.id);
        let proof_context = proof_context_for_paths(&ctx, source.id, target.id, guard_id, &paths)?;
        let source_files = source_files_for_paths(&source, &target, guard.as_ref(), &paths);
        let result = CodeItemCallPathResult {
            source_id: source.id,
            target_id: target.id,
            guard_id,
            source_file_path: NodeFilepath::new(source.rel_path.display().to_string()),
            target_file_path: NodeFilepath::new(target.rel_path.display().to_string()),
            guard_file_path: guard
                .as_ref()
                .map(|guard| NodeFilepath::new(guard.rel_path.display().to_string())),
            reachable: !paths.is_empty(),
            guarded,
            max_depth,
            max_paths,
            paths,
            violations,
            source_files,
            proof_context,
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
            .with_field("source_files", result.source_files.len().to_string())
            .with_field("proof_context", result.proof_context.len().to_string())
            .with_field("max_depth", result.max_depth.to_string())
            .with_field("max_paths", result.max_paths.to_string());
        let ui_payload = if let Some(guard_id) = result.guard_id {
            ui_payload
                .with_field("guard_id", guard_id.to_string())
                .with_field("guarded", result.guarded.unwrap_or(false).to_string())
                .with_field("violations", result.violations.len().to_string())
        } else {
            ui_payload
        };
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

fn source_files_for_paths(
    source: &lookup_support::ResolvedToolItem,
    target: &lookup_support::ResolvedToolItem,
    guard: Option<&lookup_support::ResolvedToolItem>,
    paths: &[CallPathInfo],
) -> Vec<NodeFilepath> {
    let mut files = BTreeSet::new();
    files.insert(source.rel_path.display().to_string());
    files.insert(target.rel_path.display().to_string());
    if let Some(guard) = guard {
        files.insert(guard.rel_path.display().to_string());
    }
    for path in paths {
        for node in &path.nodes {
            files.insert(node.file_path.as_ref().to_string());
        }
    }
    files.into_iter().map(NodeFilepath::new).collect()
}

fn proof_context_for_paths(
    ctx: &super::Ctx,
    source_id: uuid::Uuid,
    target_id: uuid::Uuid,
    guard_id: Option<uuid::Uuid>,
    paths: &[CallPathInfo],
) -> Result<Vec<ProofContextInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    let Some(rag) = ctx.state.rag.as_ref() else {
        return Ok(Vec::new());
    };
    if rag.proof_context_degraded() {
        return Ok(Vec::new());
    }

    let mut ids = BTreeMap::new();
    ids.insert(source_id, ());
    ids.insert(target_id, ());
    if let Some(guard_id) = guard_id {
        ids.insert(guard_id, ());
    }
    for path in paths {
        ids.insert(path.start_id, ());
        ids.insert(path.end_id, ());
        for node in &path.nodes {
            ids.insert(node.id, ());
        }
    }

    let mut rows = BTreeMap::new();
    for node_id in ids.into_keys() {
        for row in rag.exact_proof_context(node_id).map_err(|err| {
            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                "failed to collect proof context for call path node {node_id}: {err}"
            )))
        })? {
            rows.entry(row.fact_id.clone()).or_insert(row);
        }
    }

    Ok(rows.into_values().collect())
}
