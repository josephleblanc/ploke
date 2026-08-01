use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use ploke_eval::setup_client::{RunSetupPreview, RunSetupReceipt, RunSetupRequest};
use ploke_eval::walk_client::{
    AdvertisedStep, EvaluationRunCoordinate, EvaluationTraceIndex, EvaluationTraceSnapshot,
    LlmTraceCoordinate, LlmTraceIndex, LlmTraceSnapshot, OperationId, SessionId,
    WalkConfigSnapshot, WalkPhase, WalkQuerySnapshot, WalkResponse,
};

pub(crate) const DEFAULT_QUERY: &str = "::relations";
pub(crate) const MAX_TABLE_ROWS: usize = 200;
pub(crate) const RUN_LABEL_MAX_CHARS: usize = 48;
pub(crate) const WALK_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const DB_QUERY_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const TRACE_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const OPERATION_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const OPERATION_POLL_INTERVAL: Duration = Duration::from_millis(500);
pub(crate) const HANDOFF_WAIT_TIMEOUT: Duration = Duration::from_secs(40);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum CenterView {
    #[default]
    Trace,
    LiveLlm,
    Config,
    Setup,
    Query,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct UiButtonState {
    pub(crate) run_details: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum ServiceStatus {
    Unresolved,
    Offline,
    Online,
    Busy(String),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WalkRequestKind {
    Health,
    Show,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WalkRequestToken {
    pub(crate) generation: u64,
    pub(crate) kind: WalkRequestKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TraceRequestToken {
    pub(crate) generation: u64,
    pub(crate) serial: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SetupToken {
    pub(crate) generation: u64,
    pub(crate) serial: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OperationToken {
    pub(crate) generation: u64,
    pub(crate) serial: u64,
    pub(crate) operation: Option<OperationId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OperationIntent {
    Start {
        target: WalkPhase,
    },
    Step {
        advertised: AdvertisedStep,
        auto: bool,
    },
    Stop,
}

impl OperationIntent {
    pub(crate) const fn is_auto(&self) -> bool {
        matches!(self, Self::Step { auto: true, .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SetupKind {
    Preview,
    Admit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SetupPending {
    pub(crate) token: SetupToken,
    pub(crate) kind: SetupKind,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum OperationStage {
    Awaiting,
    PollAt(Instant),
}

#[derive(Debug, Clone)]
pub(crate) struct PendingOperation {
    pub(crate) token: OperationToken,
    pub(crate) intent: OperationIntent,
    pub(crate) stage: OperationStage,
}

#[derive(Debug, Clone)]
pub(crate) struct HandoffWait {
    pub(crate) prior: SessionId,
    pub(crate) started: Instant,
    pub(crate) next: Instant,
    pub(crate) delay: Duration,
    pub(crate) after: u64,
    pub(crate) continuation: HandoffContinuation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HandoffContinuation {
    Manual,
    Auto,
    Paused,
}

#[derive(Debug, Clone, Default)]
pub(crate) enum AutoMode {
    #[default]
    Idle,
    Refreshing {
        after: u64,
    },
    LoadingConfig {
        requested: bool,
        session: SessionId,
        continuation: HandoffContinuation,
    },
    CheckingSuccessor {
        after: u64,
        session: SessionId,
        continuation: HandoffContinuation,
    },
    Running,
    Stopping,
    Paused,
    Waiting(HandoffWait),
    Halted(String),
    Complete,
}

pub(crate) enum UiEvent {
    Walk {
        token: WalkRequestToken,
        result: WalkRequestResult,
    },
    Query {
        generation: u64,
        result: Result<WalkQuerySnapshot, String>,
    },
    Config {
        generation: u64,
        result: Result<Box<WalkConfigSnapshot>, String>,
    },
    SetupPreview {
        token: SetupToken,
        request: Box<RunSetupRequest>,
        result: Result<Box<RunSetupPreview>, String>,
    },
    SetupAdmit {
        token: SetupToken,
        request: Box<RunSetupRequest>,
        result: Result<Box<RunSetupReceipt>, String>,
    },
    Operation {
        token: OperationToken,
        result: Result<Box<WalkResponse>, String>,
    },
    TraceIndex {
        token: TraceRequestToken,
        result: Result<EvaluationTraceIndex, String>,
    },
    Trace {
        token: TraceRequestToken,
        coordinate: EvaluationRunCoordinate,
        result: Result<Box<EvaluationTraceSnapshot>, String>,
    },
    LlmIndex {
        token: TraceRequestToken,
        result: Result<LlmTraceIndex, String>,
    },
    LlmTrace {
        token: TraceRequestToken,
        coordinate: LlmTraceCoordinate,
        result: Result<Box<LlmTraceSnapshot>, String>,
    },
}

pub(crate) enum WalkRequestResult {
    Response(Box<WalkResponse>),
    Offline(PathBuf),
    TimedOut,
    ClientError(String),
}

impl WalkRequestKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Show => "show",
        }
    }
}

pub(crate) fn nonempty_path(text: &str) -> Option<PathBuf> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
}

pub(crate) fn optional_text(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}
