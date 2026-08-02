use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    ops::Deref,
};

use ploke_core::{
    rag_types::{CallReachEffectInfo, NodeFilepath, ProofContextInfo},
    tool_descriptions::ToolDescription,
    tool_types::ToolName,
};
use ploke_db::CallPathOptions;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tools::{
    Tool, ToolError, ToolErrorCode, ToolInvocationError, code_item_endpoint, lookup_support,
};
use code_item_endpoint::{
    CodeItemEndpoint, CodeItemEndpointOwned, endpoint_to_owned, resolve_endpoint, validate_endpoint,
};

const ITEM_DESC: &str = "Exact Rust code item coordinate.";
const EFFECT_CLASS_DESC: &str =
    "Exact admitted effect class to classify, such as async_task_spawn.";
const MAX_DEPTH_DESC: &str = "Maximum resolved call-path depth. Defaults to 3.";
const MAX_PATHS_DESC: &str = "Maximum number of paths to inspect. Defaults to 64.";

lazy_static::lazy_static! {
    static ref CODE_ITEM_EFFECT_GUARD_PARAMETERS: serde_json::Value = serde_json::json!({
        "type": "object",
        "properties": {
            "owner": { "$ref": "#/$defs/code_item_endpoint", "description": ITEM_DESC },
            "guard": { "$ref": "#/$defs/code_item_endpoint", "description": ITEM_DESC },
            "effect_class": { "type": "string", "description": EFFECT_CLASS_DESC },
            "max_depth": { "type": "integer", "minimum": 1, "description": MAX_DEPTH_DESC },
            "max_paths": { "type": "integer", "minimum": 1, "description": MAX_PATHS_DESC }
        },
        "required": ["owner", "guard", "effect_class"],
        "additionalProperties": false,
        "$defs": {
            "code_item_endpoint": code_item_endpoint::schema_property()
        }
    });
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodeItemEffectGuardParams<'a> {
    #[serde(borrow)]
    pub owner: CodeItemEndpoint<'a>,
    #[serde(borrow)]
    pub guard: CodeItemEndpoint<'a>,
    #[serde(borrow)]
    pub effect_class: Cow<'a, str>,
    #[serde(default)]
    pub max_depth: Option<u32>,
    #[serde(default)]
    pub max_paths: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tool_contracts", derive(Deserialize))]
pub struct CodeItemEffectGuardParamsOwned {
    pub owner: CodeItemEndpointOwned,
    pub guard: CodeItemEndpointOwned,
    pub effect_class: String,
    pub max_depth: Option<u32>,
    pub max_paths: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeItemEffectGuardResult {
    pub owner_id: Uuid,
    pub guard_id: Uuid,
    pub owner_file_path: NodeFilepath,
    pub guard_file_path: NodeFilepath,
    pub effect_class: String,
    pub guarded: bool,
    pub max_depth: u32,
    pub max_paths: usize,
    pub effects: Vec<CallReachEffectInfo>,
    pub violations: Vec<CallReachEffectInfo>,
    pub source_files: Vec<NodeFilepath>,
    pub proof_context: Vec<ProofContextInfo>,
}

pub struct CodeItemEffectGuard;

impl Tool for CodeItemEffectGuard {
    type Output = CodeItemEffectGuardResult;

    type OwnedParams = CodeItemEffectGuardParamsOwned;

    type Params<'de>
        = CodeItemEffectGuardParams<'de>
    where
        Self: 'de;

    fn name() -> ToolName {
        ToolName::CodeItemEffectGuard
    }

    fn description() -> ToolDescription {
        Self::name().description()
    }

    fn schema() -> &'static serde_json::Value {
        CODE_ITEM_EFFECT_GUARD_PARAMETERS.deref()
    }

    fn adapt_error(err: ToolInvocationError) -> ToolError {
        match err {
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Ui { message },
            )) => ToolError::new(
                ToolName::CodeItemEffectGuard,
                ToolErrorCode::InvalidFormat,
                message,
            )
            .retry_hint(lookup_support::LOOKUP_RETRY_HINT),
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Io { message },
            )) => ToolError::new(ToolName::CodeItemEffectGuard, ToolErrorCode::Io, message)
                .retry_hint(lookup_support::LOOKUP_RETRY_HINT),
            other => other.into_tool_error(ToolName::CodeItemEffectGuard),
        }
    }

    fn build(_ctx: &super::Ctx) -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams {
        CodeItemEffectGuardParamsOwned {
            owner: endpoint_to_owned(&params.owner),
            guard: endpoint_to_owned(&params.guard),
            effect_class: params.effect_class.clone().into_owned(),
            max_depth: params.max_depth,
            max_paths: params.max_paths,
        }
    }

    fn deserialize_params<'a>(json: &'a str) -> Result<Self::Params<'a>, ToolInvocationError> {
        let params: CodeItemEffectGuardParams<'a> =
            serde_json::from_str(json).map_err(|source| ToolInvocationError::Deserialize {
                source,
                raw: Some(json.to_string()),
            })?;
        lookup_support::validate_module_path(Self::name(), params.owner.module_path.as_ref())?;
        lookup_support::validate_module_path(Self::name(), params.guard.module_path.as_ref())?;
        Ok(params)
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: super::Ctx,
    ) -> Result<super::ToolResult, ploke_error::Error> {
        use ploke_error::{DomainError, InternalError};

        ctx.state.is_stale_err().await?;
        validate_endpoint("owner", &params.owner)?;
        validate_endpoint("guard", &params.guard)?;
        let effect_class = params.effect_class.trim();
        if effect_class.is_empty() {
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: "effect_class must not be empty.".to_string(),
            }));
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
                        "No workspace is loaded; load a workspace before using code_item_effect_guard."
                            .to_string(),
                })
            })?;

        let owner = resolve_endpoint(&ctx, &primary_root, &policy, &params.owner)?;
        let guard = resolve_endpoint(&ctx, &primary_root, &policy, &params.guard)?;
        let options = CallPathOptions {
            max_depth,
            max_paths,
        };
        let (guarded, effects, violations) = match ctx.state.rag.as_ref() {
            Some(rag) if !rag.call_context_degraded() => {
                let report = rag
                    .exact_call_effect_guard_report_for_owner(
                        owner.id,
                        guard.id,
                        effect_class,
                        options,
                    )
                    .map_err(|err| {
                        ploke_error::Error::Internal(InternalError::CompilerError(format!(
                            "failed to collect effect guard report for {} through {} and effect_class `{}`: {err}",
                            owner.id, guard.id, effect_class
                        )))
                    })?;
                match report {
                    Some(report) => (report.guarded, report.effects, report.violations),
                    None => (false, Vec::new(), Vec::new()),
                }
            }
            _ => (false, Vec::new(), Vec::new()),
        };
        let source_files = source_files_for_effects(&owner, &guard, &effects, &violations);
        let proof_context =
            proof_context_for_effects(&ctx, owner.id, guard.id, &effects, &violations)?;
        let result = CodeItemEffectGuardResult {
            owner_id: owner.id,
            guard_id: guard.id,
            owner_file_path: NodeFilepath::new(owner.rel_path.display().to_string()),
            guard_file_path: NodeFilepath::new(guard.rel_path.display().to_string()),
            effect_class: effect_class.to_string(),
            guarded,
            max_depth,
            max_paths,
            effects,
            violations,
            source_files,
            proof_context,
        };
        let summary = if result.guarded {
            format!("All {} effect(s) are guarded", result.effects.len())
        } else {
            format!(
                "{} of {} effect(s) violate the guard",
                result.violations.len(),
                result.effects.len()
            )
        };
        let ui_payload = super::ToolUiPayload::new(Self::name(), ctx.call_id.clone(), summary)
            .with_field("owner_id", result.owner_id.to_string())
            .with_field("guard_id", result.guard_id.to_string())
            .with_field("effect_class", result.effect_class.as_str())
            .with_field("guarded", result.guarded.to_string())
            .with_field("effects", result.effects.len().to_string())
            .with_field("violations", result.violations.len().to_string())
            .with_field("source_files", result.source_files.len().to_string())
            .with_field("proof_context", result.proof_context.len().to_string())
            .with_field("max_depth", result.max_depth.to_string())
            .with_field("max_paths", result.max_paths.to_string());
        let content = serde_json::to_string(&result).map_err(|err| {
            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                "failed to serialize CodeItemEffectGuardResult: {err}. This indicates an error in the ploke application itself, not due to incorrect search terms. Please consider filing an issue on the ploke github."
            )))
        })?;

        Ok(super::ToolResult {
            content,
            ui_payload: Some(ui_payload),
        })
    }
}

fn source_files_for_effects(
    owner: &lookup_support::ResolvedToolItem,
    guard: &lookup_support::ResolvedToolItem,
    effects: &[CallReachEffectInfo],
    violations: &[CallReachEffectInfo],
) -> Vec<NodeFilepath> {
    let mut files = BTreeSet::new();
    files.insert(owner.rel_path.display().to_string());
    files.insert(guard.rel_path.display().to_string());
    for effect in effects.iter().chain(violations) {
        for path in &effect.paths_to_owner {
            for node in &path.nodes {
                files.insert(node.file_path.as_ref().to_string());
            }
        }
    }
    files.into_iter().map(NodeFilepath::new).collect()
}

fn proof_context_for_effects(
    ctx: &super::Ctx,
    owner_id: Uuid,
    guard_id: Uuid,
    effects: &[CallReachEffectInfo],
    violations: &[CallReachEffectInfo],
) -> Result<Vec<ProofContextInfo>, ploke_error::Error> {
    use ploke_error::InternalError;

    let Some(rag) = ctx.state.rag.as_ref() else {
        return Ok(Vec::new());
    };
    if rag.proof_context_degraded() {
        return Ok(Vec::new());
    }

    let mut ids = BTreeMap::new();
    ids.insert(owner_id, ());
    ids.insert(guard_id, ());
    for effect in effects.iter().chain(violations) {
        ids.insert(effect.call_site.owner_id, ());
        for path in &effect.paths_to_owner {
            ids.insert(path.start_id, ());
            ids.insert(path.end_id, ());
            for node in &path.nodes {
                ids.insert(node.id, ());
            }
        }
    }

    let mut rows = BTreeMap::new();
    for node_id in ids.into_keys() {
        for row in rag.exact_proof_context(node_id).map_err(|err| {
            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                "failed to collect proof context for effect guard node {node_id}: {err}"
            )))
        })? {
            rows.entry(row.fact_id.clone()).or_insert(row);
        }
    }

    Ok(rows.into_values().collect())
}
