use ploke_core::ArcStr;
use ploke_llm::{ApiErrorSource, LlmError};
use serde::Serialize;

use crate::tools::{Audience, ToolCallPreflightError};

use super::loop_error::{
    Diagnostics, ErrorContext, ErrorSeverity, LlmAction, LlmNextStep, LoopErrorKind, RetryStrategy,
};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolArgsFailureRealization {
    EmittedMalformedToolCall,
    ProviderRejectedToolCall,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepairAction {
    ToolArgs,
    ToolName,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "decision")]
pub enum RecoveryDecision {
    Retry {
        strategy: RetryStrategy,
        reason: ArcStr,
    },
    Repair {
        strategy: RetryStrategy,
        reason: ArcStr,
        action: RepairAction,
    },
    Abort {
        reason: ArcStr,
    },
    MaybeRetry {
        reason: ArcStr,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SemanticFailure {
    UnknownToolName {
        tool_name: ArcStr,
        allowed_tools: Vec<ArcStr>,
    },
    ProviderToolArgsInvalid {
        provider: Option<ArcStr>,
        tool_name: Option<ArcStr>,
        detail: ArcStr,
        realization: ToolArgsFailureRealization,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct SemanticLoopErrorSpec {
    pub failure: SemanticFailure,
    pub recovery: RecoveryDecision,
    pub kind: LoopErrorKind,
    pub code: ArcStr,
    pub severity: ErrorSeverity,
    pub summary: ArcStr,
    pub user_action: Option<ArcStr>,
    pub llm_action: Option<LlmAction>,
    pub context: ErrorContext,
    pub diagnostics: Option<Diagnostics>,
}

pub fn normalize_llm_error(
    err: &LlmError,
    allowed_tools: &[ArcStr],
    context: ErrorContext,
) -> Option<SemanticLoopErrorSpec> {
    normalize_unknown_tool_name(err, allowed_tools, context.clone())
        .or_else(|| normalize_provider_tool_args_invalid(err, context))
}

pub fn normalize_tool_call_preflight_error(
    preflight_error: ToolCallPreflightError,
    provider_slug: Option<ArcStr>,
    mut context: ErrorContext,
) -> SemanticLoopErrorSpec {
    context.provider = provider_slug.clone();
    context.tool_call_id = Some(preflight_error.call_id.clone());
    context.tool_name = Some(ArcStr::from(preflight_error.tool_name.as_str()));

    let summary = ArcStr::from(format!(
        "Provider emitted invalid arguments for tool `{}`.",
        preflight_error.tool_name.as_str()
    ));
    let repair_details = preflight_error.error.format_for_audience(Audience::Llm);
    let diagnostic = preflight_error.error.format_for_audience(Audience::System);
    let mut constraints = vec![
        ArcStr::from("Arguments must be strict JSON."),
        ArcStr::from("Arguments must match the tool schema."),
        ArcStr::from(format!(
            "Retry the same tool name: {}",
            preflight_error.tool_name.as_str()
        )),
    ];
    if let Some(retry_hint) = &preflight_error.error.retry_hint {
        constraints.push(ArcStr::from(format!("Hint: {retry_hint}")));
    }

    SemanticLoopErrorSpec {
        failure: SemanticFailure::ProviderToolArgsInvalid {
            provider: provider_slug,
            tool_name: Some(ArcStr::from(preflight_error.tool_name.as_str())),
            detail: ArcStr::from(diagnostic.clone()),
            realization: ToolArgsFailureRealization::EmittedMalformedToolCall,
        },
        recovery: RecoveryDecision::Repair {
            strategy: RetryStrategy::Fixed,
            reason: ArcStr::from("Invalid tool arguments require a corrected tool call"),
            action: RepairAction::ToolArgs,
        },
        kind: LoopErrorKind::ModelBehavior,
        code: ArcStr::from("TOOL_ARGS_REPAIR_REQUIRED"),
        severity: ErrorSeverity::Error,
        summary,
        user_action: Some(ArcStr::from("Request a corrected tool call and retry.")),
        llm_action: Some(LlmAction {
            next_steps: vec![LlmNextStep {
                action: ArcStr::from("repair_tool_args"),
                details: Some(ArcStr::from(repair_details)),
            }],
            constraints,
            retry_hint: Some(RetryStrategy::Fixed),
        }),
        context,
        diagnostics: Some(Diagnostics {
            diagnostic: ArcStr::from(diagnostic),
        }),
    }
}

fn normalize_unknown_tool_name(
    err: &LlmError,
    allowed_tools: &[ArcStr],
    mut context: ErrorContext,
) -> Option<SemanticLoopErrorSpec> {
    let tool_name = extract_unknown_tool_name(err)?;
    context.tool_name = Some(tool_name.clone());
    let allowed_list = allowed_tools
        .iter()
        .map(|tool| tool.as_ref())
        .collect::<Vec<_>>()
        .join(", ");

    Some(SemanticLoopErrorSpec {
        failure: SemanticFailure::UnknownToolName {
            tool_name: tool_name.clone(),
            allowed_tools: allowed_tools.to_vec(),
        },
        recovery: RecoveryDecision::Repair {
            strategy: RetryStrategy::Fixed,
            reason: ArcStr::from("Model must choose a supported tool name"),
            action: RepairAction::ToolName,
        },
        kind: LoopErrorKind::ModelBehavior,
        code: ArcStr::from("UNKNOWN_TOOL_NAME"),
        severity: ErrorSeverity::Error,
        summary: ArcStr::from(format!("Unknown tool name `{}`.", tool_name)),
        user_action: Some(ArcStr::from("Retry with a supported tool.")),
        llm_action: Some(LlmAction {
            next_steps: vec![LlmNextStep {
                action: ArcStr::from("retry_tool_call_with_valid_tool_name"),
                details: Some(ArcStr::from(format!("Use one of: {allowed_list}"))),
            }],
            constraints: vec![ArcStr::from(format!(
                "tool_name must be one of: {allowed_list}"
            ))],
            retry_hint: Some(RetryStrategy::Fixed),
        }),
        context,
        diagnostics: Some(Diagnostics {
            diagnostic: ArcStr::from(err.diagnostic()),
        }),
    })
}

fn normalize_provider_tool_args_invalid(
    err: &LlmError,
    mut context: ErrorContext,
) -> Option<SemanticLoopErrorSpec> {
    let LlmError::Api {
        message,
        body_snippet,
        provider_slug,
        error_source,
        ..
    } = err
    else {
        return None;
    };
    if *error_source != ApiErrorSource::ChoiceError {
        return None;
    }
    let provider_slug = provider_slug.as_ref()?;
    if provider_slug.as_ref() != "groq" {
        return None;
    }

    let mut source = None;
    for candidate in [
        message.as_str(),
        body_snippet.as_deref().unwrap_or_default(),
    ] {
        if candidate.contains("tool call validation failed")
            || candidate.contains("parameters for tool ")
        {
            source = Some(candidate);
            break;
        }
    }
    let source = source?;

    let tool_name = source
        .split("parameters for tool ")
        .nth(1)
        .and_then(|tail| tail.split(" did not match schema").next())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ArcStr::from);

    let detail = if let Some((_, tail)) = source.split_once("did not match schema:") {
        ArcStr::from(format!(
            "Tool arguments rejected by provider: {}",
            tail.trim()
        ))
    } else {
        ArcStr::from(source.trim())
    };

    context.provider = Some(provider_slug.clone());
    context.tool_name = tool_name.clone();

    let summary = match tool_name.as_ref() {
        Some(tool_name) => ArcStr::from(format!(
            "Provider rejected arguments for tool `{tool_name}`."
        )),
        None => ArcStr::from("Provider rejected tool arguments."),
    };
    let mut constraints = vec![
        ArcStr::from("Arguments must be strict JSON."),
        ArcStr::from("Arguments must match the tool schema."),
    ];
    if let Some(tool_name) = tool_name.as_ref() {
        constraints.push(ArcStr::from(format!(
            "Retry the same tool name: {tool_name}"
        )));
    }

    Some(SemanticLoopErrorSpec {
        failure: SemanticFailure::ProviderToolArgsInvalid {
            provider: Some(provider_slug.clone()),
            tool_name: tool_name.clone(),
            detail: detail.clone(),
            realization: ToolArgsFailureRealization::ProviderRejectedToolCall,
        },
        recovery: RecoveryDecision::Repair {
            strategy: RetryStrategy::Fixed,
            reason: ArcStr::from("Invalid tool arguments require a corrected tool call"),
            action: RepairAction::ToolArgs,
        },
        kind: LoopErrorKind::ModelBehavior,
        code: ArcStr::from("TOOL_ARGS_REPAIR_REQUIRED"),
        severity: ErrorSeverity::Error,
        summary,
        user_action: Some(ArcStr::from("Request a corrected tool call and retry.")),
        llm_action: Some(LlmAction {
            next_steps: vec![LlmNextStep {
                action: ArcStr::from("repair_tool_args"),
                details: Some(detail.clone()),
            }],
            constraints,
            retry_hint: Some(RetryStrategy::Fixed),
        }),
        context,
        diagnostics: Some(Diagnostics { diagnostic: detail }),
    })
}

fn extract_unknown_tool_name(err: &LlmError) -> Option<ArcStr> {
    let message = match err {
        LlmError::Deserialization { message, .. } => message.as_str(),
        _ => return None,
    };
    let needle = "unknown variant `";
    if let Some(start) = message.find(needle) {
        let rest = &message[start + needle.len()..];
        if let Some(end) = rest.find('`') {
            return Some(ArcStr::from(rest[..end].to_string()));
        }
    }
    let needle = "unknown variant \"";
    if let Some(start) = message.find(needle) {
        let rest = &message[start + needle.len()..];
        if let Some(end) = rest.find('"') {
            return Some(ArcStr::from(rest[..end].to_string()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_provider_tool_args_invalid_maps_groq_choice_error_to_repair() {
        let err = LlmError::Api {
            status: 200,
            message: "Upstream error from Groq: tool call validation failed: parameters for tool cargo did not match schema: errors: [`/command`: value must be one of \"test\", \"check\"]".to_string(),
            url: None,
            body_snippet: None,
            api_code: Some(ArcStr::from("502")),
            provider_name: Some(ArcStr::from("Groq")),
            provider_slug: Some(ArcStr::from("groq")),
            error_source: ApiErrorSource::ChoiceError,
        };

        let spec = normalize_llm_error(&err, &[], ErrorContext::new(1, 0))
            .expect("groq validation error should normalize");

        assert_eq!(spec.code.as_ref(), "TOOL_ARGS_REPAIR_REQUIRED");
        assert!(matches!(
            spec.recovery,
            RecoveryDecision::Repair {
                action: RepairAction::ToolArgs,
                ..
            }
        ));
        assert_eq!(spec.context.provider.as_deref(), Some("groq"));
        assert_eq!(spec.context.tool_name.as_deref(), Some("cargo"));
    }

    #[test]
    fn normalize_unknown_tool_name_maps_to_repair() {
        let err = LlmError::Deserialization {
            message: "unknown variant `fake_tool`, expected one of `read_file`".to_string(),
            body_snippet: None,
        };
        let allowed = vec![ArcStr::from("read_file"), ArcStr::from("list_dir")];

        let spec = normalize_llm_error(&err, &allowed, ErrorContext::new(1, 0))
            .expect("unknown tool should normalize");

        assert_eq!(spec.code.as_ref(), "UNKNOWN_TOOL_NAME");
        assert!(matches!(
            spec.recovery,
            RecoveryDecision::Repair {
                action: RepairAction::ToolName,
                ..
            }
        ));
        assert_eq!(spec.context.tool_name.as_deref(), Some("fake_tool"));
    }
}
