//! Read-only historical replay cursor for Prototype 1 walk sessions.
//!
//! The cursor is an operator/debug projection over durable journal evidence. It
//! never calls providers, spawns children, mutates worktrees, appends transition
//! journal entries, or seals History. `branch-live` writes only an explicit
//! provenance/admission record so later live branching has a durable source.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    cli::prototype1_state::{
        cli_facing::{campaign_manifest_path_for_id, prototype1_state_transition_error},
        event::RecordedAt,
        identity::load_parent_identity_optional,
        journal::{self, JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
        walk::phase::WalkPhase,
    },
    spec::PrepareError,
};

// ANCHOR: prototype1_replay_cursor
/// In-memory replay cursor over an immutable snapshot of journal entries.
#[derive(Debug, Clone)]
pub(crate) struct ReplayCursor {
    repo_root: PathBuf,
    campaign_id: ploke_records::ids::CampaignId,
    journal_path: PathBuf,
    steps: Vec<ReplayStep>,
    cursor: Option<usize>,
}

/// One replayable durable transition projection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ReplayStep {
    pub(crate) index: usize,
    pub(crate) recorded_at: Option<RecordedAt>,
    pub(crate) phase: Option<WalkPhase>,
    pub(crate) kind: String,
    pub(crate) subject: String,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BranchProvenanceRecord {
    pub(crate) schema_version: String,
    pub(crate) recorded_at: RecordedAt,
    pub(crate) source_campaign_id: ploke_records::ids::CampaignId,
    pub(crate) source_repo_root: PathBuf,
    pub(crate) source_journal_path: PathBuf,
    pub(crate) cursor_index: usize,
    pub(crate) cursor_kind: String,
    pub(crate) cursor_phase: Option<WalkPhase>,
    pub(crate) cursor_subject: String,
    pub(crate) cursor_detail: String,
    pub(crate) reason: String,
    pub(crate) admission: String,
}
// ANCHOR_END: prototype1_replay_cursor

impl ReplayCursor {
    pub(crate) fn load(repo_root: &Path) -> Result<Self, PrepareError> {
        let identity = load_parent_identity_optional(repo_root)?.ok_or_else(|| {
            PrepareError::InvalidBatchSelection {
                detail: format!(
                    "walk replay requires a parent identity at '{}'",
                    repo_root.display()
                ),
            }
        })?;
        let campaign_id = identity.campaign_id().clone();
        let manifest_path = campaign_manifest_path_for_id(&campaign_id)?;
        let journal_path = prototype1_transition_journal_path(&manifest_path);
        let journal = PrototypeJournal::new(journal_path.clone());
        let entries = journal.load_entries().map_err(|error| {
            prototype1_state_transition_error("prototype1_replay_journal", error.to_string())
        })?;
        let steps = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| replay_step(index, entry))
            .collect::<Vec<_>>();
        let cursor = steps.len().checked_sub(1);
        Ok(Self {
            repo_root: repo_root.to_path_buf(),
            campaign_id,
            journal_path,
            steps,
            cursor,
        })
    }

    pub(crate) fn render(&self, tail: usize) -> String {
        let mut lines = Vec::new();
        let campaign_dir = campaign_dir_from_journal(&self.journal_path);
        lines.push("historical replay cursor (read-only)".to_string());
        lines.push(
            "mode: inspecting durable journal evidence; the command header phase is live walk-server state, not replay position"
                .to_string(),
        );
        lines.push(format!("campaign_id: {}", self.campaign_id));
        lines.push(format!("repo_root: {}", self.repo_root.display()));
        lines.push(format!("campaign_dir: {}", display_dir(&campaign_dir)));
        lines.push(format!(
            "journal_path: {}",
            display_under(&self.journal_path, &campaign_dir, "campaign_dir")
        ));
        lines.push(format!("entries: {}", self.steps.len()));
        lines.push(format!(
            "cursor: {}",
            self.cursor
                .map(|index| format!(
                    "#{index} of #{} (operator cursor)",
                    self.steps.len().saturating_sub(1)
                ))
                .unwrap_or_else(|| "-".to_string())
        ));
        if let Some(current) = self.current() {
            lines.push("current:".to_string());
            lines.push(format!(
                "  entry: #{:04} {} {} {}",
                current.index,
                current.phase.map(|phase| phase.as_str()).unwrap_or("-"),
                display_kind(current),
                current.subject
            ));
            lines.push(format!("  meaning: {}", entry_meaning(current)));
            lines.push(format!("  detail: {}", current.detail));
        }
        let tail = tail.max(1);
        let (start, end) = replay_window(self.steps.len(), self.cursor, tail);
        if start < end {
            lines.push(format!(
                "entry_window: showing #{}..#{} of #{} ({} entries, includes cursor; use --tail N to expand)",
                start,
                end - 1,
                self.steps.len().saturating_sub(1),
                end - start
            ));
        } else {
            lines.push("entry_window: (journal is empty)".to_string());
        }
        for step in &self.steps[start..end] {
            let marker = if Some(step.index) == self.cursor {
                "*"
            } else {
                " "
            };
            lines.push(format!(
                "{marker} #{:04} {:>4} {:<20} {}",
                step.index,
                step.phase.map(|phase| phase.as_str()).unwrap_or("-"),
                display_kind(step),
                step.subject
            ));
            if Some(step.index) == self.cursor {
                lines.push(format!("      {}", entry_meaning(step)));
            }
        }
        lines.push("note: replay/back/forward move only this operator cursor; they do not undo durable side effects".to_string());
        lines.join("\n")
    }

    pub(crate) fn back(&mut self, steps: usize) {
        if self.steps.is_empty() {
            self.cursor = None;
            return;
        }
        let current = self.cursor.unwrap_or(self.steps.len() - 1);
        self.cursor = Some(current.saturating_sub(steps.max(1)));
    }

    pub(crate) fn forward(&mut self, steps: usize) {
        if self.steps.is_empty() {
            self.cursor = None;
            return;
        }
        let current = self.cursor.unwrap_or(0);
        let last = self.steps.len() - 1;
        self.cursor = Some(current.saturating_add(steps.max(1)).min(last));
    }

    pub(crate) fn jump(&mut self, index: usize) -> Result<(), PrepareError> {
        if index >= self.steps.len() {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "replay cursor index {index} is out of range for {} entries",
                    self.steps.len()
                ),
            });
        }
        self.cursor = Some(index);
        Ok(())
    }

    pub(crate) fn write_branch_provenance(&self, reason: String) -> Result<PathBuf, PrepareError> {
        let current = self
            .current()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "cannot branch from replay: journal cursor is empty".to_string(),
            })?;
        if reason.trim().is_empty() {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "walk branch-live requires a non-empty --reason".to_string(),
            });
        }
        let manifest_path = campaign_manifest_path_for_id(&self.campaign_id)?;
        let dir = manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("prototype1")
            .join("replay-branches");
        fs::create_dir_all(&dir).map_err(|source| PrepareError::WriteManifest {
            path: dir.clone(),
            source,
        })?;
        let recorded_at = RecordedAt::now();
        let path = dir.join(format!(
            "replay-branch-{}-{:04}.json",
            recorded_at.0, current.index
        ));
        let record = BranchProvenanceRecord {
            schema_version: "prototype1.replay_branch_provenance.v1".to_string(),
            recorded_at,
            source_campaign_id: self.campaign_id.clone(),
            source_repo_root: self.repo_root.clone(),
            source_journal_path: self.journal_path.clone(),
            cursor_index: current.index,
            cursor_kind: current.kind.clone(),
            cursor_phase: current.phase,
            cursor_subject: current.subject.clone(),
            cursor_detail: current.detail.clone(),
            reason,
            admission: "walk branch-live --allow provenance-record".to_string(),
        };
        let json = serde_json::to_vec_pretty(&record).map_err(PrepareError::Serialize)?;
        fs::write(&path, json).map_err(|source| PrepareError::WriteManifest {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }

    fn current(&self) -> Option<&ReplayStep> {
        self.cursor.and_then(|index| self.steps.get(index))
    }
}

fn replay_window(len: usize, cursor: Option<usize>, tail: usize) -> (usize, usize) {
    if len == 0 {
        return (0, 0);
    }
    let tail = tail.max(1).min(len);
    let cursor = cursor.unwrap_or(len - 1).min(len - 1);
    let mut start = cursor.saturating_sub(tail / 2);
    if start + tail > len {
        start = len - tail;
    }
    (start, start + tail)
}

fn display_kind(step: &ReplayStep) -> &str {
    match step.kind.as_str() {
        "resource.parentcomplete" => "parent_complete",
        "resource.parentstart" => "parent_start_resource",
        other => other,
    }
}

fn entry_meaning(step: &ReplayStep) -> &'static str {
    match step.kind.as_str() {
        "parent_started" => "parent runtime started",
        "resource.parentstart" => {
            "parent-start resource was recorded for reconstruction/audit evidence"
        }
        "resource.parentcomplete" => {
            "parent turn reached final report/parent-complete evidence; campaign may still be non-terminal"
        }
        "child_artifact_committed" => "child artifact/diff was committed for a planned child",
        "materialize_branch" => "child branch materialization event",
        "build_child" => "child runtime build event",
        "spawn_child" => "child runtime spawn event",
        "child_ready" => "child runtime reported ready",
        "observe_child" => "parent observed child runtime result",
        "successor.selected" => "successor selection was recorded",
        "successor.stopped" => "continuation stopped without successor handoff",
        "successor_handoff" | "successor.spawned" | "successor.checkout" | "successor.ready" => {
            "successor handoff/install/spawn evidence"
        }
        _ => "durable transition-journal evidence",
    }
}

fn campaign_dir_from_journal(path: &Path) -> PathBuf {
    path.parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn display_dir(path: &Path) -> String {
    let mut value = path.display().to_string();
    if !value.ends_with(std::path::MAIN_SEPARATOR) {
        value.push(std::path::MAIN_SEPARATOR);
    }
    value
}

fn display_under(path: &Path, root: &Path, label: &str) -> String {
    if let Ok(stripped) = path.strip_prefix(root) {
        if stripped.as_os_str().is_empty() {
            return format!("{{{label}}}/");
        }
        return format!("{{{label}}}/{}", stripped.display());
    }
    path.display().to_string()
}

fn replay_step(index: usize, entry: &JournalEntry) -> ReplayStep {
    let (recorded_at, phase, kind, subject, detail) = match entry {
        JournalEntry::ParentStarted(entry) => (
            Some(entry.recorded_at),
            Some(WalkPhase::R5),
            "parent_started".to_string(),
            entry.parent_identity.node_id().to_string(),
            format!(
                "parent runtime started at repo_root '{}' pid={} handoff_runtime={}",
                entry.repo_root.display(),
                entry.pid,
                entry
                    .handoff_runtime_id
                    .map(|runtime| runtime.to_string())
                    .unwrap_or_else(|| "-".to_string())
            ),
        ),
        JournalEntry::Resource(sample) => {
            let phase = match sample.phase {
                journal::resource::Phase::ParentStart => WalkPhase::R5,
                journal::resource::Phase::ParentComplete => WalkPhase::R14a,
            };
            (
                Some(sample.recorded_at),
                Some(phase),
                format!("resource.{:?}", sample.phase).to_ascii_lowercase(),
                sample.node_id.clone(),
                format!(
                    "{:?} {:?} at '{}' status={:?} bytes={}",
                    sample.phase,
                    sample.subject,
                    sample.path.display(),
                    sample.status,
                    sample
                        .bytes
                        .map(|bytes| bytes.to_string())
                        .unwrap_or_else(|| "-".to_string())
                ),
            )
        }
        JournalEntry::ChildArtifactCommitted(entry) => (
            Some(entry.recorded_at),
            Some(WalkPhase::R8),
            "child_artifact_committed".to_string(),
            entry.node_id.clone(),
            format!(
                "child_branch={} target={} target_commit={}",
                entry.child_branch,
                entry.target_relpath.display(),
                entry.target_commit
            ),
        ),
        JournalEntry::ActiveCheckoutAdvanced(entry) => (
            Some(entry.recorded_at),
            Some(WalkPhase::R13b),
            "active_checkout_advanced".to_string(),
            entry.selected_parent_identity.node_id().to_string(),
            format!(
                "selected_branch={} installed_commit={} active_parent_root={}",
                entry.selected_branch,
                entry.installed_commit,
                entry.active_parent_root.display()
            ),
        ),
        JournalEntry::SuccessorHandoff(entry) => (
            Some(entry.recorded_at),
            Some(WalkPhase::R13b),
            "successor_handoff".to_string(),
            entry.node_id.clone(),
            format!(
                "runtime={} pid={} ready_path={} binary={}",
                entry.runtime_id,
                entry.pid,
                entry.ready_path.display(),
                entry.binary_path.display()
            ),
        ),
        JournalEntry::Successor(record) => successor_step(record),
        JournalEntry::MaterializeBranch(entry) => (
            Some(entry.recorded_at),
            Some(WalkPhase::R8),
            "materialize_branch".to_string(),
            entry.refs.node_id.clone(),
            format!(
                "transition_id={} branch_id={} phase={:?}",
                entry.transition_id, entry.refs.branch_id, entry.phase
            ),
        ),
        JournalEntry::BuildChild(entry) => (
            Some(entry.recorded_at),
            Some(WalkPhase::R11),
            "build_child".to_string(),
            entry.refs.node_id.clone(),
            format!(
                "phase={:?} result={:?} binary={}",
                entry.phase,
                entry.result,
                entry.paths.binary_path.display()
            ),
        ),
        JournalEntry::SpawnChild(entry) => (
            Some(entry.recorded_at),
            Some(WalkPhase::R11),
            "spawn_child".to_string(),
            entry.refs.node_id.clone(),
            format!(
                "phase={:?} runtime={} child_pid={:?} result={:?}",
                entry.phase, entry.runtime_id, entry.child_pid, entry.result
            ),
        ),
        JournalEntry::Child(record) => (
            None,
            Some(WalkPhase::R11),
            record.entry_kind().to_string(),
            record.runtime_id().to_string(),
            format!("runtime={}", record.runtime_id()),
        ),
        JournalEntry::ChildReady(entry) => (
            Some(entry.recorded_at),
            Some(WalkPhase::R11),
            "child_ready".to_string(),
            entry.refs.node_id.clone(),
            format!("runtime={} pid={}", entry.runtime_id, entry.pid),
        ),
        JournalEntry::ObserveChild(entry) => (
            Some(entry.recorded_at),
            Some(WalkPhase::R11),
            "observe_child".to_string(),
            entry.refs.node_id.clone(),
            format!("result={:?} runtime={}", entry.result, entry.runtime_id),
        ),
    };
    ReplayStep {
        index,
        recorded_at,
        phase,
        kind,
        subject,
        detail,
    }
}

fn successor_step(
    record: &crate::cli::prototype1_state::successor::Record,
) -> (
    Option<RecordedAt>,
    Option<WalkPhase>,
    String,
    String,
    String,
) {
    use crate::cli::prototype1_state::successor::State;
    match &record.state {
        State::Selected {
            decision,
            selection_decision,
        } => (
            Some(record.recorded_at),
            Some(WalkPhase::R13b),
            "successor.selected".to_string(),
            record.node_id.clone(),
            format!(
                "disposition={:?} selected_next_branch={:?} selection_outcome={:?}",
                decision.disposition,
                decision.selected_next_branch_id,
                selection_decision.as_ref().map(|decision| decision.outcome)
            ),
        ),
        State::Stopped {
            decision,
            selection_decision,
        } => (
            Some(record.recorded_at),
            Some(WalkPhase::R13a),
            "successor.stopped".to_string(),
            record.node_id.clone(),
            format!(
                "disposition={:?} selection_outcome={:?}",
                decision.disposition,
                selection_decision.as_ref().map(|decision| decision.outcome)
            ),
        ),
        State::Spawned {
            pid,
            active_parent_root,
            binary_path,
            invocation_path,
            ready_path,
            ..
        } => (
            Some(record.recorded_at),
            Some(WalkPhase::R13b),
            "successor.spawned".to_string(),
            record.node_id.clone(),
            format!(
                "runtime={} pid={} root={} binary={} invocation={} ready={}",
                record
                    .runtime_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                pid,
                active_parent_root.display(),
                binary_path.display(),
                invocation_path.display(),
                ready_path.display()
            ),
        ),
        State::Checkout {
            phase,
            active_parent_root,
            selected_branch,
            installed_commit,
        } => (
            Some(record.recorded_at),
            Some(WalkPhase::R13b),
            "successor.checkout".to_string(),
            record.node_id.clone(),
            format!(
                "phase={phase:?} branch={} commit={} root={}",
                selected_branch,
                installed_commit.as_deref().unwrap_or("-"),
                active_parent_root.display()
            ),
        ),
        State::Ready { pid, ready_path } => (
            Some(record.recorded_at),
            Some(WalkPhase::R13b),
            "successor.ready".to_string(),
            record.node_id.clone(),
            format!(
                "runtime={} pid={} ready={}",
                record
                    .runtime_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                pid,
                ready_path.display()
            ),
        ),
        State::TimedOut {
            waited_ms,
            ready_path,
        } => (
            Some(record.recorded_at),
            Some(WalkPhase::R13b),
            "successor.timed_out".to_string(),
            record.node_id.clone(),
            format!(
                "runtime={} waited_ms={} ready={}",
                record
                    .runtime_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                waited_ms,
                ready_path.display()
            ),
        ),
        State::ExitedBeforeReady { exit_code } => (
            Some(record.recorded_at),
            Some(WalkPhase::R13b),
            "successor.exited_before_ready".to_string(),
            record.node_id.clone(),
            format!(
                "runtime={} exit_code={exit_code:?}",
                record
                    .runtime_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "-".to_string())
            ),
        ),
        State::Completed {
            status,
            completion_path,
            trace_path,
            detail,
        } => (
            Some(record.recorded_at),
            Some(WalkPhase::R14b),
            "successor.completed".to_string(),
            record.node_id.clone(),
            format!(
                "runtime={} status={status:?} completion={} trace={} detail={}",
                record
                    .runtime_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                completion_path.display(),
                trace_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "-".to_string()),
                detail.as_deref().unwrap_or("-")
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_records::ids::CampaignId;

    fn step(index: usize, kind: &str) -> ReplayStep {
        ReplayStep {
            index,
            recorded_at: None,
            phase: Some(if kind == "resource.parentcomplete" {
                WalkPhase::R14a
            } else {
                WalkPhase::R5
            }),
            kind: kind.to_string(),
            subject: format!("node-{index}"),
            detail: format!("detail-{index}"),
        }
    }

    #[test]
    fn replay_window_includes_index_zero_cursor() {
        assert_eq!(replay_window(10, Some(0), 3), (0, 3));
        assert_eq!(replay_window(10, Some(9), 3), (7, 10));
        assert_eq!(replay_window(10, Some(5), 3), (4, 7));
    }

    #[test]
    fn render_replay_index_window_uses_cursor_not_recent_tail() {
        let cursor = ReplayCursor {
            repo_root: PathBuf::from("/tmp/repo"),
            campaign_id: CampaignId::from("campaign-replay-test"),
            journal_path: PathBuf::from(
                "/tmp/eval/campaigns/campaign-replay-test/prototype1/transition-journal.jsonl",
            ),
            steps: (0..5)
                .map(|index| {
                    if index == 4 {
                        step(index, "resource.parentcomplete")
                    } else {
                        step(index, "parent_started")
                    }
                })
                .collect(),
            cursor: Some(0),
        };

        let rendered = cursor.render(3);

        assert!(rendered.contains("cursor: #0 of #4 (operator cursor)"));
        assert!(rendered.contains("entry_window: showing #0..#2 of #4"));
        assert!(rendered.contains("* #0000"));
        assert!(!rendered.contains("#0004"));
        assert!(rendered.contains("command header phase is live walk-server state"));
    }

    #[test]
    fn parent_complete_entry_has_plain_language_meaning() {
        let step = step(2, "resource.parentcomplete");

        assert_eq!(display_kind(&step), "parent_complete");
        assert!(entry_meaning(&step).contains("campaign may still be non-terminal"));
    }
}
