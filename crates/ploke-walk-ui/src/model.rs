use std::{path::PathBuf, time::Duration};

use ploke_eval::walk_client::{WalkQuerySnapshot, WalkResponse};

pub(crate) const DEFAULT_QUERY: &str = "::relations";
pub(crate) const MAX_TABLE_ROWS: usize = 200;
pub(crate) const RUN_LABEL_MAX_CHARS: usize = 48;
pub(crate) const WALK_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const DB_QUERY_TIMEOUT: Duration = Duration::from_secs(30);

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

pub(crate) enum UiEvent {
    Walk {
        token: WalkRequestToken,
        result: WalkRequestResult,
    },
    Query {
        generation: u64,
        result: Result<WalkQuerySnapshot, String>,
    },
}

pub(crate) enum WalkRequestResult {
    Response(WalkResponse),
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
