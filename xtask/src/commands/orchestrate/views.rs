use std::collections::BTreeSet;

use clap::ValueEnum;

use crate::commands::XtaskError;

use super::board::{ACTIVE_STALE_AFTER_SECS, REVIEW_STALE_AFTER_SECS, parse_time};
use super::{Board, Task, TaskState};

/// Built-in task views.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TaskView {
    /// All tasks.
    All,
    /// Active tasks.
    Active,
    /// Not-started tasks without open blockers.
    Ready,
    /// Completed tasks awaiting review.
    Review,
    /// Blocked tasks.
    Blocked,
    /// Stale active or review tasks.
    Stale,
}

/// Read-only task query for status projections.
#[derive(Debug, Clone)]
pub(super) struct TaskQuery {
    task_set: Option<String>,
    view: Option<TaskView>,
    filters: Vec<TaskFilter>,
    filter_labels: Vec<String>,
}

#[derive(Debug, Clone)]
struct TaskFilter {
    negated: bool,
    kind: TaskFilterKind,
}

#[derive(Debug, Clone)]
enum TaskFilterKind {
    Set(String),
    Lane(String),
    State(TaskStateFilter),
    Worker(String),
    Blocked(bool),
    Stale(bool),
    NoWorker,
}

#[derive(Debug, Clone, Copy)]
enum TaskStateFilter {
    NotStarted,
    AssignedQueued,
    AssignedActive,
    CompleteUnreviewed,
    CompleteReviewed,
    Blocked,
}

impl TaskQuery {
    pub(super) fn new(
        task_set: Option<String>,
        view: Option<TaskView>,
        raw_filters: Vec<String>,
    ) -> Result<Self, XtaskError> {
        let mut filters = Vec::new();
        for raw in &raw_filters {
            filters.push(TaskFilter::parse(raw)?);
        }
        Ok(Self {
            task_set,
            view,
            filters,
            filter_labels: raw_filters,
        })
    }

    pub(super) fn all() -> Self {
        Self {
            task_set: None,
            view: None,
            filters: Vec::new(),
            filter_labels: Vec::new(),
        }
    }

    pub(super) fn task_set(&self) -> Option<&str> {
        self.task_set.as_deref()
    }

    pub(super) fn view_label(&self) -> Option<&'static str> {
        self.view.map(TaskView::label)
    }

    pub(super) fn filter_labels(&self) -> &[String] {
        &self.filter_labels
    }

    pub(super) fn has_filters(&self) -> bool {
        self.task_set.is_some() || self.view.is_some() || !self.filters.is_empty()
    }
}

pub(super) fn filter_tasks(
    board: &Board,
    query: &TaskQuery,
) -> Result<Option<BTreeSet<String>>, XtaskError> {
    if let Some(task_set) = query.task_set() {
        board.ensure_task_set(task_set)?;
    }
    for filter in &query.filters {
        if let TaskFilterKind::Set(set) = &filter.kind {
            board.ensure_task_set(set)?;
        }
    }

    if query.task_set.is_none() && query.view.is_none() && query.filters.is_empty() {
        return Ok(None);
    }

    Ok(Some(
        board
            .tasks
            .values()
            .filter(|task| query.matches(board, task))
            .map(|task| task.id.clone())
            .collect(),
    ))
}

impl TaskQuery {
    fn matches(&self, board: &Board, task: &Task) -> bool {
        if let Some(task_set) = self.task_set() {
            if !task.task_sets.iter().any(|set| set == task_set) {
                return false;
            }
        }
        if let Some(view) = self.view {
            if !view.matches(board, task) {
                return false;
            }
        }
        self.filters
            .iter()
            .all(|filter| filter.matches(board, task))
    }
}

impl TaskView {
    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Active => "active",
            Self::Ready => "ready",
            Self::Review => "review",
            Self::Blocked => "blocked",
            Self::Stale => "stale",
        }
    }

    fn matches(self, board: &Board, task: &Task) -> bool {
        match self {
            Self::All => true,
            Self::Active => matches!(task.state, TaskState::AssignedActive { .. }),
            Self::Ready => {
                matches!(task.state, TaskState::NotStarted)
                    && board.open_blockers_for_task(&task.id).is_empty()
            }
            Self::Review => matches!(task.state, TaskState::CompleteUnreviewed { .. }),
            Self::Blocked => {
                matches!(task.state, TaskState::Blocked { .. })
                    || !board.open_blockers_for_task(&task.id).is_empty()
            }
            Self::Stale => task_is_stale(task),
        }
    }
}

impl TaskFilter {
    fn parse(raw: &str) -> Result<Self, XtaskError> {
        let (negated, body) = raw
            .strip_prefix('-')
            .map_or((false, raw), |stripped| (true, stripped));
        let kind = if let Some(set) = body.strip_prefix("set:") {
            TaskFilterKind::Set(required_value(raw, set)?)
        } else if let Some(lane) = body.strip_prefix("lane:") {
            TaskFilterKind::Lane(required_value(raw, lane)?)
        } else if let Some(state) = body.strip_prefix("state:") {
            TaskFilterKind::State(TaskStateFilter::parse(required_value(raw, state)?)?)
        } else if let Some(worker) = body.strip_prefix("worker:") {
            TaskFilterKind::Worker(required_value(raw, worker)?)
        } else if let Some(blocked) = body.strip_prefix("blocked:") {
            TaskFilterKind::Blocked(parse_bool(raw, blocked)?)
        } else if let Some(stale) = body.strip_prefix("stale:") {
            TaskFilterKind::Stale(parse_bool(raw, stale)?)
        } else if body == "no:worker" {
            TaskFilterKind::NoWorker
        } else {
            return Err(XtaskError::validation(format!(
                "Unsupported task filter `{raw}`"
            ))
            .with_recovery(
                "Use set:<name>, lane:<name>, state:<state>, worker:<id>, blocked:true, stale:true, no:worker, or prefix with `-` for negation.",
            ));
        };
        Ok(Self { negated, kind })
    }

    fn matches(&self, board: &Board, task: &Task) -> bool {
        let matched = match &self.kind {
            TaskFilterKind::Set(set) => task.task_sets.iter().any(|task_set| task_set == set),
            TaskFilterKind::Lane(lane) => task.lane == *lane,
            TaskFilterKind::State(state) => state.matches(&task.state),
            TaskFilterKind::Worker(worker) => task_worker(task).as_deref() == Some(worker.as_str()),
            TaskFilterKind::Blocked(expected) => {
                let blocked = matches!(task.state, TaskState::Blocked { .. })
                    || !board.open_blockers_for_task(&task.id).is_empty();
                blocked == *expected
            }
            TaskFilterKind::Stale(expected) => task_is_stale(task) == *expected,
            TaskFilterKind::NoWorker => task_worker(task).is_none(),
        };
        matched != self.negated
    }
}

impl TaskStateFilter {
    fn parse(raw: String) -> Result<Self, XtaskError> {
        match raw.as_str() {
            "not-started" | "not_started" | "ready" => Ok(Self::NotStarted),
            "queued" | "assigned-queued" | "assigned_queued" => Ok(Self::AssignedQueued),
            "active" | "assigned-active" | "assigned_active" => Ok(Self::AssignedActive),
            "review" | "complete-unreviewed" | "complete_unreviewed" => {
                Ok(Self::CompleteUnreviewed)
            }
            "reviewed" | "complete-reviewed" | "complete_reviewed" => Ok(Self::CompleteReviewed),
            "blocked" => Ok(Self::Blocked),
            _ => Err(
                XtaskError::validation(format!("Unsupported task state `{raw}`"))
                    .with_recovery("Use ready, queued, active, review, reviewed, or blocked."),
            ),
        }
    }

    fn matches(self, state: &TaskState) -> bool {
        matches!(
            (self, state),
            (Self::NotStarted, TaskState::NotStarted)
                | (Self::AssignedQueued, TaskState::AssignedQueued { .. })
                | (Self::AssignedActive, TaskState::AssignedActive { .. })
                | (
                    Self::CompleteUnreviewed,
                    TaskState::CompleteUnreviewed { .. }
                )
                | (Self::CompleteReviewed, TaskState::CompleteReviewed)
                | (Self::Blocked, TaskState::Blocked { .. })
        )
    }
}

fn task_worker(task: &Task) -> Option<String> {
    match &task.state {
        TaskState::AssignedQueued { worker } | TaskState::AssignedActive { worker } => {
            Some(worker.clone())
        }
        TaskState::CompleteUnreviewed { worker } => worker.clone(),
        _ => None,
    }
}

fn task_is_stale(task: &Task) -> bool {
    match &task.state {
        TaskState::AssignedActive { .. } => {
            let timestamp = task.activated_at.as_deref().unwrap_or(&task.updated_at);
            age_seconds(timestamp).is_some_and(|age| age > ACTIVE_STALE_AFTER_SECS)
        }
        TaskState::CompleteUnreviewed { .. } => {
            let timestamp = task.completed_at.as_deref().unwrap_or(&task.updated_at);
            age_seconds(timestamp).is_some_and(|age| age > REVIEW_STALE_AFTER_SECS)
        }
        _ => false,
    }
}

fn age_seconds(timestamp: &str) -> Option<i64> {
    parse_time(timestamp).map(|then| (chrono::Utc::now() - then).num_seconds())
}

fn required_value(raw: &str, value: &str) -> Result<String, XtaskError> {
    if value.is_empty() {
        Err(
            XtaskError::validation(format!("Missing value in filter `{raw}`"))
                .with_recovery("Use a non-empty filter value."),
        )
    } else {
        Ok(value.to_string())
    }
}

fn parse_bool(raw: &str, value: &str) -> Result<bool, XtaskError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(
            XtaskError::validation(format!("Filter `{raw}` expects true or false"))
                .with_recovery("Use true or false."),
        ),
    }
}
