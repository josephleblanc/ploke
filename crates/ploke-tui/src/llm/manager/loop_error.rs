use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ploke_core::ArcStr;
use ploke_llm::{LlmError, response::FinishReason};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Clone, Debug, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopErrorKind {
    Transport,
    HttpStatus,
    ProviderProtocol,
    ModelBehavior,
    ToolExecution,
    StateMachine,
    SafetyPolicy,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Fatal,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryStrategy {
    Fixed,
    Backoff,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryAdvice {
    No {
        reason: ArcStr,
    },
    Maybe {
        reason: ArcStr,
    },
    Yes {
        strategy: RetryStrategy,
        reason: ArcStr,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitPhase {
    PreCommit,
    MessageCommitted,
    ToolResultsCommitted,
    SideEffectsCommitted,
}

#[derive(Clone, Debug, Serialize)]
pub struct ErrorContext {
    pub provider: Option<ArcStr>,
    pub model: Option<ArcStr>,
    pub endpoint: Option<ArcStr>,
    pub request_id: Option<Uuid>,
    pub finish_reason: Option<FinishReason>,
    pub native_finish_reason: Option<ArcStr>,
    pub tool_name: Option<ArcStr>,
    pub tool_call_id: Option<ArcStr>,
    pub phase: Option<ArcStr>,
    pub attempt: u32,
    pub chain_index: usize,
}

impl ErrorContext {
    pub fn new(attempt: u32, chain_index: usize) -> Self {
        Self {
            provider: None,
            model: None,
            endpoint: None,
            request_id: None,
            finish_reason: None,
            native_finish_reason: None,
            tool_name: None,
            tool_call_id: None,
            phase: None,
            attempt,
            chain_index,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LlmNextStep {
    pub action: ArcStr,
    pub details: Option<ArcStr>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LlmAction {
    pub next_steps: Vec<LlmNextStep>,
    pub constraints: Vec<ArcStr>,
    pub retry_hint: Option<RetryStrategy>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostics {
    pub diagnostic: ArcStr,
}

#[derive(Clone, Debug, Serialize)]
pub struct LoopError {
    pub error_id: Uuid,
    pub fingerprint: u64,
    pub kind: LoopErrorKind,
    pub code: ArcStr,
    pub severity: ErrorSeverity,
    pub retry: RetryAdvice,
    pub commit_phase: CommitPhase,
    pub summary: ArcStr,
    pub user_action: Option<ArcStr>,
    pub llm_action: Option<LlmAction>,
    pub context: ErrorContext,
    pub diagnostics: Option<Diagnostics>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutcome {
    Completed,
    Aborted { error_id: Uuid },
    Exhausted { error_id: Uuid },
}

#[derive(Clone, Debug, Serialize)]
pub struct ChatSessionReport {
    pub session_id: Uuid,
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub assistant_message_id: Uuid,
    pub outcome: SessionOutcome,
    pub errors: Vec<LoopError>,
    pub commit_phase: CommitPhase,
    pub attempts: u32,
}

impl ChatSessionReport {
    pub fn new(
        session_id: Uuid,
        request_id: Uuid,
        parent_id: Uuid,
        assistant_message_id: Uuid,
    ) -> Self {
        Self {
            session_id,
            request_id,
            parent_id,
            assistant_message_id,
            outcome: SessionOutcome::Completed,
            errors: Vec::new(),
            commit_phase: CommitPhase::PreCommit,
            attempts: 0,
        }
    }

    pub fn record_error(&mut self, error: LoopError) {
        self.errors.push(error);
    }

    pub fn last_error(&self) -> Option<&LoopError> {
        self.errors.last()
    }

    pub fn summary(&self) -> String {
        match &self.outcome {
            SessionOutcome::Completed => "Request summary: [success]".to_string(),
            SessionOutcome::Aborted { error_id } => {
                format!("Request summary: [aborted] error_id={error_id}")
            }
            SessionOutcome::Exhausted { error_id } => {
                format!("Request summary: [exhausted] error_id={error_id}")
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum ErrorAudience {
    User,
    Llm,
    System,
}

#[derive(Clone, Debug)]
pub enum Verbosity {
    Terse,
    Normal,
    Verbose,
}

#[derive(Clone, Debug)]
pub struct ErrorView {
    pub summary: String,
    pub details: Option<String>,
    pub llm_payload: Option<serde_json::Value>,
}

pub fn render_error_view(
    error: &LoopError,
    audience: ErrorAudience,
    verbosity: Verbosity,
) -> ErrorView {
    match audience {
        ErrorAudience::User => {
            let summary = format!("Error: {}", error.summary);
            let mut details = None;
            if matches!(verbosity, Verbosity::Normal | Verbosity::Verbose)
                && let Some(action) = &error.user_action
            {
                details = Some(format!("Suggested action: {}", action));
            }
            if matches!(verbosity, Verbosity::Verbose) {
                let context = format_error_context(error);
                details = Some(match details {
                    Some(existing) => format!("{existing}\n{context}"),
                    None => context,
                });
            }
            ErrorView {
                summary,
                details,
                llm_payload: None,
            }
        }
        ErrorAudience::System => {
            let summary = format!("Loop error {}: {}", error.code, error.summary);
            let mut details = Some(format_error_context(error));
            if matches!(verbosity, Verbosity::Verbose)
                && let Some(diag) = &error.diagnostics
            {
                details = Some(format!(
                    "{}\nDiagnostics: {}",
                    details.unwrap_or_default(),
                    diag.diagnostic
                ));
            }
            ErrorView {
                summary,
                details,
                llm_payload: None,
            }
        }
        ErrorAudience::Llm => ErrorView {
            summary: error.summary.to_string(),
            details: None,
            llm_payload: Some(build_llm_payload(error)),
        },
    }
}

pub fn classify_llm_error(
    err: &LlmError,
    mut context: ErrorContext,
    commit_phase: CommitPhase,
) -> LoopError {
    let (kind, code, severity, retry, user_action, llm_action) = match err {
        LlmError::Request { is_timeout, .. } => {
            let code = if *is_timeout {
                ArcStr::from("TRANSPORT_TIMEOUT")
            } else {
                ArcStr::from("TRANSPORT_REQUEST_FAILED")
            };
            let retry = if *is_timeout {
                RetryAdvice::Yes {
                    strategy: RetryStrategy::Backoff,
                    reason: ArcStr::from("Transient timeout"),
                }
            } else {
                RetryAdvice::Maybe {
                    reason: ArcStr::from("Transient network error"),
                }
            };
            (
                LoopErrorKind::Transport,
                code,
                ErrorSeverity::Error,
                retry,
                Some(ArcStr::from("Check network connectivity and retry.")),
                None,
            )
        }
        LlmError::Api { status, .. } => {
            let code = ArcStr::from(format!("HTTP_{}", status));
            let retry = match *status {
                429 => RetryAdvice::Yes {
                    strategy: RetryStrategy::Backoff,
                    reason: ArcStr::from("Rate limited"),
                },
                500 | 502 | 503 | 504 => RetryAdvice::Yes {
                    strategy: RetryStrategy::Backoff,
                    reason: ArcStr::from("Provider error"),
                },
                _ => RetryAdvice::Maybe {
                    reason: ArcStr::from("Provider returned an error"),
                },
            };
            let action = match *status {
                429 => Some(ArcStr::from("Wait briefly, then retry.")),
                401 | 403 => Some(ArcStr::from("Verify API credentials and retry.")),
                _ => None,
            };
            (
                LoopErrorKind::HttpStatus,
                code,
                ErrorSeverity::Warning,
                retry,
                action,
                None,
            )
        }
        LlmError::RateLimited => (
            LoopErrorKind::HttpStatus,
            ArcStr::from("HTTP_429"),
            ErrorSeverity::Warning,
            RetryAdvice::Yes {
                strategy: RetryStrategy::Backoff,
                reason: ArcStr::from("Rate limited"),
            },
            Some(ArcStr::from("Wait briefly, then retry.")),
            None,
        ),
        LlmError::Authentication => (
            LoopErrorKind::HttpStatus,
            ArcStr::from("AUTH_FAILED"),
            ErrorSeverity::Error,
            RetryAdvice::No {
                reason: ArcStr::from("Authentication failed"),
            },
            Some(ArcStr::from("Check API credentials and retry.")),
            None,
        ),
        LlmError::Timeout => (
            LoopErrorKind::Transport,
            ArcStr::from("PROVIDER_TIMEOUT"),
            ErrorSeverity::Warning,
            RetryAdvice::Yes {
                strategy: RetryStrategy::Backoff,
                reason: ArcStr::from("Provider timeout"),
            },
            Some(ArcStr::from("Retry or switch provider.")),
            None,
        ),
        LlmError::ContentFilter => (
            LoopErrorKind::SafetyPolicy,
            ArcStr::from("CONTENT_FILTERED"),
            ErrorSeverity::Warning,
            RetryAdvice::No {
                reason: ArcStr::from("Content filtered"),
            },
            Some(ArcStr::from("Rephrase the request and retry.")),
            None,
        ),
        LlmError::Serialization(_) => (
            LoopErrorKind::StateMachine,
            ArcStr::from("REQUEST_SERIALIZATION_FAILED"),
            ErrorSeverity::Error,
            RetryAdvice::No {
                reason: ArcStr::from("Failed to serialize request"),
            },
            None,
            None,
        ),
        LlmError::Deserialization { .. } => (
            LoopErrorKind::ProviderProtocol,
            ArcStr::from("RESPONSE_DESERIALIZATION_FAILED"),
            ErrorSeverity::Error,
            RetryAdvice::Maybe {
                reason: ArcStr::from("Invalid provider response"),
            },
            None,
            None,
        ),
        LlmError::ToolCall(_) => (
            LoopErrorKind::ToolExecution,
            ArcStr::from("TOOL_EXECUTION_FAILED"),
            ErrorSeverity::Error,
            RetryAdvice::No {
                reason: ArcStr::from("Tool execution failed"),
            },
            Some(ArcStr::from(
                "Review tool output and retry with corrected input.",
            )),
            Some(LlmAction {
                next_steps: vec![LlmNextStep {
                    action: ArcStr::from("repair_tool_args"),
                    details: None,
                }],
                constraints: vec![ArcStr::from("Arguments must be strict JSON.")],
                retry_hint: None,
            }),
        ),
        LlmError::Conversion(_) | LlmError::Unknown(_) | LlmError::Embedding(_) => (
            LoopErrorKind::StateMachine,
            ArcStr::from("INTERNAL_ERROR"),
            ErrorSeverity::Error,
            RetryAdvice::Maybe {
                reason: ArcStr::from("Unexpected internal error"),
            },
            None,
            None,
        ),
        LlmError::ChatStep(_) => (
            LoopErrorKind::ModelBehavior,
            ArcStr::from("INVALID_MODEL_RESPONSE"),
            ErrorSeverity::Error,
            RetryAdvice::Maybe {
                reason: ArcStr::from("Model response did not match expectations"),
            },
            None,
            None,
        ),
        LlmError::FinishError { finish_reason, .. } => {
            context.finish_reason = Some(finish_reason.clone());
            finish_reason_metadata(finish_reason)
        }
    };

    let summary = ArcStr::from(err.to_string());
    let diagnostics = Some(Diagnostics {
        diagnostic: ArcStr::from(err.diagnostic()),
    });

    LoopError {
        error_id: Uuid::new_v4(),
        fingerprint: fingerprint_for(&kind, &code, &context),
        kind,
        code,
        severity,
        retry,
        commit_phase,
        summary,
        user_action,
        llm_action,
        context,
        diagnostics,
    }
}

pub fn classify_finish_reason(
    finish_reason: &FinishReason,
    mut context: ErrorContext,
    commit_phase: CommitPhase,
) -> LoopError {
    context.finish_reason = Some(finish_reason.clone());
    let (kind, code, severity, retry, user_action, llm_action) =
        finish_reason_metadata(finish_reason);
    let summary = finish_reason_summary(finish_reason);

    LoopError {
        error_id: Uuid::new_v4(),
        fingerprint: fingerprint_for(&kind, &code, &context),
        kind,
        code,
        severity,
        retry,
        commit_phase,
        summary,
        user_action,
        llm_action,
        context,
        diagnostics: None,
    }
}

fn fingerprint_for(kind: &LoopErrorKind, code: &ArcStr, context: &ErrorContext) -> u64 {
    let mut hasher = DefaultHasher::new();
    kind.hash(&mut hasher);
    code.hash(&mut hasher);
    context.provider.hash(&mut hasher);
    context.model.hash(&mut hasher);
    context.tool_name.hash(&mut hasher);
    hasher.finish()
}

fn format_error_context(error: &LoopError) -> String {
    let mut lines = vec![
        format!("kind: {:?}", error.kind),
        format!("severity: {:?}", error.severity),
        format!("retry: {:?}", error.retry),
        format!("commit_phase: {:?}", error.commit_phase),
    ];
    if let Some(model) = &error.context.model {
        lines.push(format!("model: {}", model));
    }
    if let Some(provider) = &error.context.provider {
        lines.push(format!("provider: {}", provider));
    }
    if let Some(tool) = &error.context.tool_name {
        lines.push(format!("tool_name: {}", tool));
    }
    if let Some(reason) = &error.context.finish_reason {
        lines.push(format!("finish_reason: {:?}", reason));
    }
    lines.join("\n")
}

fn build_llm_payload(error: &LoopError) -> serde_json::Value {
    let next_steps = error
        .llm_action
        .as_ref()
        .map(|action| {
            action
                .next_steps
                .iter()
                .map(|step| {
                    json!({
                        "action": step.action,
                        "details": step.details,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let constraints = error
        .llm_action
        .as_ref()
        .map(|action| action.constraints.clone())
        .unwrap_or_default();
    let retry = match &error.retry {
        RetryAdvice::No { reason } => json!({ "allowed": false, "reason": reason }),
        RetryAdvice::Maybe { reason } => json!({ "allowed": true, "reason": reason }),
        RetryAdvice::Yes { strategy, reason } => {
            json!({ "allowed": true, "strategy": strategy_str(strategy), "reason": reason })
        }
    };
    json!({
        "type": "ploke.error",
        "error_id": error.error_id,
        "code": error.code,
        "kind": kind_str(&error.kind),
        "summary": error.summary,
        "where": {
            "phase": error.context.phase,
            "tool_name": error.context.tool_name,
            "provider": error.context.provider,
            "model": error.context.model,
            "finish_reason": error.context.finish_reason,
        },
        "retry": retry,
        "next_steps": next_steps,
        "constraints": constraints,
        "more_info": {
            "diagnostic": error.diagnostics.as_ref().map(|d| d.diagnostic.clone())
        }
    })
}

fn finish_reason_metadata(
    finish_reason: &FinishReason,
) -> (
    LoopErrorKind,
    ArcStr,
    ErrorSeverity,
    RetryAdvice,
    Option<ArcStr>,
    Option<LlmAction>,
) {
    match finish_reason {
        FinishReason::ContentFilter => (
            LoopErrorKind::SafetyPolicy,
            ArcStr::from("CONTENT_FILTERED"),
            ErrorSeverity::Warning,
            RetryAdvice::No {
                reason: ArcStr::from("Content filtered"),
            },
            Some(ArcStr::from("Rephrase the request and retry.")),
            None,
        ),
        FinishReason::Length => (
            LoopErrorKind::ModelBehavior,
            ArcStr::from("OUTPUT_TRUNCATED"),
            ErrorSeverity::Warning,
            RetryAdvice::No {
                reason: ArcStr::from("Output length exhausted"),
            },
            Some(ArcStr::from(
                "Ask for a shorter response or request continuation.",
            )),
            Some(LlmAction {
                next_steps: vec![LlmNextStep {
                    action: ArcStr::from("continue_output"),
                    details: None,
                }],
                constraints: Vec::new(),
                retry_hint: None,
            }),
        ),
        FinishReason::Timeout => (
            LoopErrorKind::Transport,
            ArcStr::from("PROVIDER_TIMEOUT"),
            ErrorSeverity::Warning,
            RetryAdvice::Maybe {
                reason: ArcStr::from("Provider timeout"),
            },
            Some(ArcStr::from("Retry or switch provider.")),
            None,
        ),
        FinishReason::Error(_) => (
            LoopErrorKind::ProviderProtocol,
            ArcStr::from("PROVIDER_ERROR"),
            ErrorSeverity::Error,
            RetryAdvice::Maybe {
                reason: ArcStr::from("Provider returned an error"),
            },
            None,
            None,
        ),
        FinishReason::ToolCalls | FinishReason::Stop => (
            LoopErrorKind::StateMachine,
            ArcStr::from("UNEXPECTED_FINISH_REASON"),
            ErrorSeverity::Error,
            RetryAdvice::No {
                reason: ArcStr::from("Unexpected finish reason"),
            },
            None,
            None,
        ),
    }
}

fn finish_reason_summary(finish_reason: &FinishReason) -> ArcStr {
    match finish_reason {
        FinishReason::Error(msg) => ArcStr::from(format!("Finish reason error: {}", msg)),
        FinishReason::Length => ArcStr::from("Finish reason length: response truncated."),
        FinishReason::Timeout => ArcStr::from("Finish reason timeout from provider."),
        FinishReason::ContentFilter => ArcStr::from("Finish reason content filter."),
        FinishReason::ToolCalls => ArcStr::from("Finish reason tool calls."),
        FinishReason::Stop => ArcStr::from("Finish reason stop."),
    }
}

fn kind_str(kind: &LoopErrorKind) -> &'static str {
    match kind {
        LoopErrorKind::Transport => "transport",
        LoopErrorKind::HttpStatus => "http_status",
        LoopErrorKind::ProviderProtocol => "provider_protocol",
        LoopErrorKind::ModelBehavior => "model_behavior",
        LoopErrorKind::ToolExecution => "tool_execution",
        LoopErrorKind::StateMachine => "state_machine",
        LoopErrorKind::SafetyPolicy => "safety_policy",
    }
}

fn strategy_str(strategy: &RetryStrategy) -> &'static str {
    match strategy {
        RetryStrategy::Fixed => "fixed",
        RetryStrategy::Backoff => "backoff",
    }
}
