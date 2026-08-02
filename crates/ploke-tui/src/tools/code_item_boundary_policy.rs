use std::{collections::BTreeSet, ops::Deref};

use ploke_core::{
    rag_types::{
        CrateBoundaryPolicyViolationInfo, ModuleBoundaryPolicyViolationInfo, NodeFilepath,
    },
    tool_descriptions::ToolDescription,
    tool_types::ToolName,
};
use ploke_db::{CallPathOptions, CrateBoundaryPolicyRule, ModuleBoundaryPolicyRule};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tools::{
    Tool, ToolError, ToolErrorCode, ToolInvocationError, code_item_endpoint, lookup_support,
};
use code_item_endpoint::{
    CodeItemEndpoint, CodeItemEndpointOwned, endpoint_to_owned, resolve_endpoint, validate_endpoint,
};

const ITEM_DESC: &str = "Exact Rust code item coordinate.";
const RULE_DESC: &str =
    "Forbidden module-boundary rule evaluated over resolved call graph boundary edges.";
const CRATE_RULE_DESC: &str =
    "Forbidden crate-boundary rule evaluated over resolved call graph boundary edges.";
const RULE_ID_DESC: &str = "Stable rule identifier returned with each matching violation.";
const PREFIX_DESC: &str =
    "Module path prefix as segments, for example [\"crate\", \"ext_traits\"].";
const CRATE_DESC: &str = "Exact source crate name.";
const MAX_DEPTH_DESC: &str = "Maximum resolved call-path depth. Defaults to 3.";
const MAX_PATHS_DESC: &str = "Maximum number of paths to inspect. Defaults to 64.";

lazy_static::lazy_static! {
    static ref CODE_ITEM_BOUNDARY_POLICY_PARAMETERS: serde_json::Value = serde_json::json!({
        "type": "object",
        "properties": {
            "owner": { "$ref": "#/$defs/code_item_endpoint", "description": ITEM_DESC },
            "rules": {
                "type": "array",
                "minItems": 1,
                "items": { "$ref": "#/$defs/boundary_policy_rule" },
                "description": RULE_DESC
            },
            "crate_rules": {
                "type": "array",
                "minItems": 1,
                "items": { "$ref": "#/$defs/crate_boundary_policy_rule" },
                "description": CRATE_RULE_DESC
            },
            "max_depth": { "type": "integer", "minimum": 1, "description": MAX_DEPTH_DESC },
            "max_paths": { "type": "integer", "minimum": 1, "description": MAX_PATHS_DESC }
        },
        "required": ["owner"],
        "additionalProperties": false,
        "$defs": {
            "code_item_endpoint": code_item_endpoint::schema_property(),
            "boundary_policy_rule": {
                "type": "object",
                "properties": {
                    "rule_id": { "type": "string", "description": RULE_ID_DESC },
                    "caller_module_prefix": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string" },
                        "description": PREFIX_DESC
                    },
                    "callee_module_prefix": {
                        "type": "array",
                        "minItems": 1,
                        "items": { "type": "string" },
                        "description": PREFIX_DESC
                    }
                },
                "required": ["rule_id", "caller_module_prefix", "callee_module_prefix"],
                "additionalProperties": false
            },
            "crate_boundary_policy_rule": {
                "type": "object",
                "properties": {
                    "rule_id": { "type": "string", "description": RULE_ID_DESC },
                    "caller_crate": { "type": "string", "description": CRATE_DESC },
                    "callee_crate": { "type": "string", "description": CRATE_DESC }
                },
                "required": ["rule_id", "caller_crate", "callee_crate"],
                "additionalProperties": false
            }
        }
    });
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryRuleParam {
    pub rule_id: String,
    pub caller_module_prefix: Vec<String>,
    pub callee_module_prefix: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateBoundaryRuleParam {
    pub rule_id: String,
    pub caller_crate: String,
    pub callee_crate: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodeItemBoundaryPolicyParams<'a> {
    #[serde(borrow)]
    pub owner: CodeItemEndpoint<'a>,
    #[serde(default)]
    pub rules: Vec<BoundaryRuleParam>,
    #[serde(default)]
    pub crate_rules: Vec<CrateBoundaryRuleParam>,
    #[serde(default)]
    pub max_depth: Option<u32>,
    #[serde(default)]
    pub max_paths: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tool_contracts", derive(Deserialize))]
pub struct CodeItemBoundaryPolicyParamsOwned {
    pub owner: CodeItemEndpointOwned,
    pub rules: Vec<BoundaryRuleParam>,
    pub crate_rules: Vec<CrateBoundaryRuleParam>,
    pub max_depth: Option<u32>,
    pub max_paths: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeItemBoundaryPolicyResult {
    pub owner_id: Uuid,
    pub owner_file_path: NodeFilepath,
    pub rules: Vec<ModuleBoundaryPolicyRule>,
    #[serde(default)]
    pub crate_rules: Vec<CrateBoundaryPolicyRule>,
    pub max_depth: u32,
    pub max_paths: usize,
    pub violations: Vec<ModuleBoundaryPolicyViolationInfo>,
    #[serde(default)]
    pub crate_violations: Vec<CrateBoundaryPolicyViolationInfo>,
    pub source_files: Vec<NodeFilepath>,
}

pub struct CodeItemBoundaryPolicy;

impl Tool for CodeItemBoundaryPolicy {
    type Output = CodeItemBoundaryPolicyResult;

    type OwnedParams = CodeItemBoundaryPolicyParamsOwned;

    type Params<'de>
        = CodeItemBoundaryPolicyParams<'de>
    where
        Self: 'de;

    fn name() -> ToolName {
        ToolName::CodeItemBoundaryPolicy
    }

    fn description() -> ToolDescription {
        Self::name().description()
    }

    fn schema() -> &'static serde_json::Value {
        CODE_ITEM_BOUNDARY_POLICY_PARAMETERS.deref()
    }

    fn adapt_error(err: ToolInvocationError) -> ToolError {
        match err {
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Ui { message },
            )) => ToolError::new(
                ToolName::CodeItemBoundaryPolicy,
                ToolErrorCode::InvalidFormat,
                message,
            )
            .retry_hint(lookup_support::LOOKUP_RETRY_HINT),
            ToolInvocationError::Exec(ploke_error::Error::Domain(
                ploke_error::DomainError::Io { message },
            )) => ToolError::new(ToolName::CodeItemBoundaryPolicy, ToolErrorCode::Io, message)
                .retry_hint(lookup_support::LOOKUP_RETRY_HINT),
            other => other.into_tool_error(ToolName::CodeItemBoundaryPolicy),
        }
    }

    fn build(_ctx: &super::Ctx) -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams {
        CodeItemBoundaryPolicyParamsOwned {
            owner: endpoint_to_owned(&params.owner),
            rules: params.rules.clone(),
            crate_rules: params.crate_rules.clone(),
            max_depth: params.max_depth,
            max_paths: params.max_paths,
        }
    }

    fn deserialize_params<'a>(json: &'a str) -> Result<Self::Params<'a>, ToolInvocationError> {
        let params: CodeItemBoundaryPolicyParams<'a> =
            serde_json::from_str(json).map_err(|source| ToolInvocationError::Deserialize {
                source,
                raw: Some(json.to_string()),
            })?;
        lookup_support::validate_module_path(Self::name(), params.owner.module_path.as_ref())?;
        Ok(params)
    }

    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: super::Ctx,
    ) -> Result<super::ToolResult, ploke_error::Error> {
        use ploke_error::{DomainError, InternalError};

        ctx.state.is_stale_err().await?;
        validate_endpoint("owner", &params.owner)?;
        let rules = rules_to_db(&params.rules)?;
        let crate_rules = crate_rules_to_db(&params.crate_rules)?;
        if rules.is_empty() && crate_rules.is_empty() {
            return Err(ploke_error::Error::Domain(DomainError::Ui {
                message: "rules or crate_rules must include at least one boundary policy rule."
                    .to_string(),
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
                        "No workspace is loaded; load a workspace before using code_item_boundary_policy."
                            .to_string(),
                })
            })?;

        let owner = resolve_endpoint(&ctx, &primary_root, &policy, &params.owner)?;
        let options = CallPathOptions {
            max_depth,
            max_paths,
        };
        let violations = match ctx.state.rag.as_ref() {
            Some(rag) if !rag.call_context_degraded() => rag
                .exact_module_boundary_policy_violations_from_owner(owner.id, options, &rules)
                .map_err(|err| {
                    ploke_error::Error::Internal(InternalError::CompilerError(format!(
                        "failed to collect module-boundary policy violations for {}: {err}",
                        owner.id
                    )))
                })?
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let crate_violations = match ctx.state.rag.as_ref() {
            Some(rag) if !rag.call_context_degraded() => rag
                .exact_crate_boundary_policy_violations_from_owner(owner.id, options, &crate_rules)
                .map_err(|err| {
                    ploke_error::Error::Internal(InternalError::CompilerError(format!(
                        "failed to collect crate-boundary policy violations for {}: {err}",
                        owner.id
                    )))
                })?
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let source_files = source_files_for(&owner, &violations, &crate_violations);
        let result = CodeItemBoundaryPolicyResult {
            owner_id: owner.id,
            owner_file_path: NodeFilepath::new(owner.rel_path.display().to_string()),
            rules,
            crate_rules,
            max_depth,
            max_paths,
            violations,
            crate_violations,
            source_files,
        };
        let summary = format!(
            "Found {} module-boundary and {} crate-boundary policy violation(s)",
            result.violations.len(),
            result.crate_violations.len()
        );
        let ui_payload = super::ToolUiPayload::new(Self::name(), ctx.call_id.clone(), summary)
            .with_field("owner_id", result.owner_id.to_string())
            .with_field("rules", result.rules.len().to_string())
            .with_field("crate_rules", result.crate_rules.len().to_string())
            .with_field("violations", result.violations.len().to_string())
            .with_field(
                "crate_violations",
                result.crate_violations.len().to_string(),
            )
            .with_field("source_files", result.source_files.len().to_string())
            .with_field("max_depth", result.max_depth.to_string())
            .with_field("max_paths", result.max_paths.to_string());
        let content = serde_json::to_string(&result).map_err(|err| {
            ploke_error::Error::Internal(InternalError::CompilerError(format!(
                "failed to serialize CodeItemBoundaryPolicyResult: {err}. This indicates an error in the ploke application itself, not due to incorrect search terms. Please consider filing an issue on the ploke github."
            )))
        })?;

        Ok(super::ToolResult {
            content,
            ui_payload: Some(ui_payload),
        })
    }
}

fn rules_to_db(
    rules: &[BoundaryRuleParam],
) -> Result<Vec<ModuleBoundaryPolicyRule>, ploke_error::Error> {
    rules
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            let rule_id = rule.rule_id.trim();
            if rule_id.is_empty() {
                return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                    message: format!("rules[{index}].rule_id must not be empty."),
                }));
            }
            Ok(ModuleBoundaryPolicyRule {
                rule_id: rule_id.to_string(),
                caller_module_prefix: clean_prefix(
                    index,
                    "caller_module_prefix",
                    &rule.caller_module_prefix,
                )?,
                callee_module_prefix: clean_prefix(
                    index,
                    "callee_module_prefix",
                    &rule.callee_module_prefix,
                )?,
            })
        })
        .collect()
}

fn crate_rules_to_db(
    rules: &[CrateBoundaryRuleParam],
) -> Result<Vec<CrateBoundaryPolicyRule>, ploke_error::Error> {
    rules
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            let rule_id = rule.rule_id.trim();
            if rule_id.is_empty() {
                return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                    message: format!("crate_rules[{index}].rule_id must not be empty."),
                }));
            }
            Ok(CrateBoundaryPolicyRule {
                rule_id: rule_id.to_string(),
                caller_crate: clean_crate(index, "caller_crate", &rule.caller_crate)?,
                callee_crate: clean_crate(index, "callee_crate", &rule.callee_crate)?,
            })
        })
        .collect()
}

fn clean_crate(index: usize, field: &str, value: &str) -> Result<String, ploke_error::Error> {
    let value = value.trim();
    if value.is_empty() {
        return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: format!("crate_rules[{index}].{field} must not be empty."),
        }));
    }
    Ok(value.to_string())
}

fn clean_prefix(
    index: usize,
    field: &str,
    segments: &[String],
) -> Result<Vec<String>, ploke_error::Error> {
    if segments.is_empty() {
        return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
            message: format!("rules[{index}].{field} must not be empty."),
        }));
    }

    segments
        .iter()
        .enumerate()
        .map(|(segment_index, segment)| {
            let segment = segment.trim();
            if segment.is_empty() {
                return Err(ploke_error::Error::Domain(ploke_error::DomainError::Ui {
                    message: format!("rules[{index}].{field}[{segment_index}] must not be empty."),
                }));
            }
            Ok(segment.to_string())
        })
        .collect()
}

fn source_files_for(
    owner: &lookup_support::ResolvedToolItem,
    violations: &[ModuleBoundaryPolicyViolationInfo],
    crate_violations: &[CrateBoundaryPolicyViolationInfo],
) -> Vec<NodeFilepath> {
    let mut files = BTreeSet::new();
    files.insert(owner.rel_path.display().to_string());
    for violation in violations {
        files.insert(violation.edge.caller.file_path.as_ref().to_string());
        files.insert(violation.edge.callee.file_path.as_ref().to_string());
    }
    for violation in crate_violations {
        files.insert(violation.edge.caller.file_path.as_ref().to_string());
        files.insert(violation.edge.callee.file_path.as_ref().to_string());
    }
    files.into_iter().map(NodeFilepath::new).collect()
}
