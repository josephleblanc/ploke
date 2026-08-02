use std::ops::Deref;

use ploke_core::{
    rag_types::{CallBuildDomainInfo, CallNodeInfo, CallTestEntrypointInfo, ProofContextInfo},
    tool_descriptions::ToolDescription,
    tool_types::ToolName,
};
use ploke_error::InternalError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tools::{Tool, ToolError, ToolErrorCode, ToolInvocationError};

const MAX_RESULTS_DESC: &str =
    "Maximum number of private uncalled nodes to return. Defaults to 128.";
const DEFAULT_MAX_RESULTS: usize = 128;

lazy_static::lazy_static! {
    static ref CODE_PRIVATE_UNCALLED_PARAMETERS: serde_json::Value = serde_json::json!({
        "type": "object",
        "properties": {
            "max_results": {
                "type": "integer",
                "minimum": 1,
                "description": MAX_RESULTS_DESC
            }
        },
        "additionalProperties": false
    });
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrivateUncalledParams {
    #[serde(default)]
    pub max_results: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tool_contracts", derive(Deserialize))]
pub struct PrivateUncalledParamsOwned {
    pub max_results: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateEntrypointSummary {
    pub node_id: Uuid,
    pub proof_context: Vec<ProofContextInfo>,
    #[serde(default)]
    pub build_domains: Vec<CallBuildDomainInfo>,
    #[serde(default)]
    pub test_entrypoints: Vec<CallTestEntrypointInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodePrivateUncalledResult {
    pub total: usize,
    pub returned: usize,
    pub truncated: bool,
    pub nodes: Vec<CallNodeInfo>,
    #[serde(default)]
    pub entrypoint_summaries: Vec<PrivateEntrypointSummary>,
}

pub struct CodePrivateUncalled;

impl Tool for CodePrivateUncalled {
    type Output = CodePrivateUncalledResult;

    type OwnedParams = PrivateUncalledParamsOwned;

    type Params<'de>
        = PrivateUncalledParams
    where
        Self: 'de;

    fn name() -> ToolName {
        ToolName::CodePrivateUncalled
    }

    fn description() -> ToolDescription {
        Self::name().description()
    }

    fn schema() -> &'static serde_json::Value {
        CODE_PRIVATE_UNCALLED_PARAMETERS.deref()
    }

    fn adapt_error(err: ToolInvocationError) -> ToolError {
        match err {
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Ui { message },
            )) => ToolError::new(
                ToolName::CodePrivateUncalled,
                ToolErrorCode::InvalidFormat,
                message,
            ),
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Io { message },
            )) => ToolError::new(ToolName::CodePrivateUncalled, ToolErrorCode::Io, message),
            other => other.into_tool_error(ToolName::CodePrivateUncalled),
        }
    }

    fn build(_ctx: &super::Ctx) -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams {
        Self::OwnedParams {
            max_results: params.max_results,
        }
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: super::Ctx,
    ) -> Result<super::ToolResult, ploke_error::Error> {
        ctx.state.is_stale_err().await?;

        let max_results = params.max_results.unwrap_or(DEFAULT_MAX_RESULTS);
        if max_results == 0 {
            return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                message: "max_results must be greater than zero.".to_string(),
            }));
        }

        let nodes = match ctx.state.rag.as_ref() {
            Some(rag) if !rag.call_context_degraded() => rag
                .exact_private_uncalled_nodes()
                .map_err(|err| {
                    ploke_error::Error::Internal(InternalError::CompilerError(format!(
                        "failed to collect private uncalled nodes: {err}"
                    )))
                })?
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let total = nodes.len();
        let returned_nodes = nodes.into_iter().take(max_results).collect::<Vec<_>>();
        let entrypoint_summaries = match ctx.state.rag.as_ref() {
            Some(rag) if !rag.proof_context_degraded() => {
                let mut summaries = Vec::new();
                for node in &returned_nodes {
                    let node_id = node.id.to_string();
                    let proof_context = rag.exact_proof_context(node.id).map_err(|err| {
                        ploke_error::Error::Internal(InternalError::CompilerError(format!(
                            "failed to collect entrypoint summaries for private uncalled node {}: {err}",
                            node.id
                        )))
                    })?;
                    let proof_context = proof_context
                        .into_iter()
                        .filter(|row| {
                            row.kind == "entrypoint_summary"
                                && row.definition_id.as_deref() == Some(node_id.as_str())
                                && row.status.as_deref() == Some("admitted")
                        })
                        .collect::<Vec<_>>();
                    let build_domains = rag
                        .exact_call_build_domains_for_node(node.id)
                        .map_err(|err| {
                            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                                "failed to collect build domains for private uncalled node {}: {err}",
                                node.id
                            )))
                        })?
                        .unwrap_or_default();
                    let test_entrypoints = rag
                        .exact_call_test_entrypoints_for_node(node.id)
                        .map_err(|err| {
                            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                                "failed to collect test entrypoints for private uncalled node {}: {err}",
                                node.id
                            )))
                        })?
                        .unwrap_or_default();
                    if !proof_context.is_empty()
                        || !build_domains.is_empty()
                        || !test_entrypoints.is_empty()
                    {
                        summaries.push(PrivateEntrypointSummary {
                            node_id: node.id,
                            proof_context,
                            build_domains,
                            test_entrypoints,
                        });
                    }
                }
                summaries
            }
            _ => Vec::new(),
        };
        let result = CodePrivateUncalledResult {
            total,
            returned: returned_nodes.len(),
            truncated: total > returned_nodes.len(),
            nodes: returned_nodes,
            entrypoint_summaries,
        };
        let entrypoint_build_domains = result
            .entrypoint_summaries
            .iter()
            .map(|summary| summary.build_domains.len())
            .sum::<usize>();
        let entrypoint_test_summaries = result
            .entrypoint_summaries
            .iter()
            .map(|summary| summary.test_entrypoints.len())
            .sum::<usize>();
        let summary = format!("Found {} private uncalled node(s)", result.total);
        let ui_payload = super::ToolUiPayload::new(Self::name(), ctx.call_id.clone(), summary)
            .with_field("total", result.total.to_string())
            .with_field("returned", result.returned.to_string())
            .with_field("truncated", result.truncated.to_string())
            .with_field("max_results", max_results.to_string())
            .with_field(
                "entrypoint_summaries",
                result.entrypoint_summaries.len().to_string(),
            )
            .with_field(
                "entrypoint_build_domains",
                entrypoint_build_domains.to_string(),
            )
            .with_field(
                "entrypoint_test_summaries",
                entrypoint_test_summaries.to_string(),
            );
        let content = serde_json::to_string(&result).map_err(|err| {
            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                "failed to serialize CodePrivateUncalledResult: {err}. This indicates an error in the ploke application itself, not due to incorrect search terms. Please consider filing an issue on the ploke github."
            )))
        })?;

        Ok(super::ToolResult {
            content,
            ui_payload: Some(ui_payload),
        })
    }
}
