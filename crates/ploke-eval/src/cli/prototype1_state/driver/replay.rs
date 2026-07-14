//! Read-only historical replay cursor for Prototype 1 walk sessions.
//!
//! The cursor is an operator/debug projection over durable journal evidence. It
//! never calls providers, spawns children, mutates worktrees, appends transition
//! journal entries, or seals History. `branch-live` writes only an explicit
//! provenance/admission record so later live branching has a durable source.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use ploke_records::ids::CampaignId;
use serde::{Deserialize, Serialize};

use crate::{
    campaign_manifest_path,
    cli::prototype1_state::{
        cli_facing::prototype1_state_transition_error,
        event::RecordedAt,
        identity::load_parent_identity_optional,
        journal::{self, JournalEntry, PrototypeJournal, prototype1_transition_journal_path},
        walk::phase::WalkPhase,
    },
    loop_graph::RuntimeId,
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

type AttemptKey = (CampaignId, String, RuntimeId);
type ParentKey = (CampaignId, String, String, u32);
type SelectionKey = (CampaignId, String);

#[derive(Default)]
struct ReplayProjection {
    spawns: BTreeMap<AttemptKey, crate::cli::prototype1_state::successor::Record>,
    acknowledged: BTreeMap<AttemptKey, journal::SuccessorHandoffEntry>,
    failed: BTreeSet<AttemptKey>,
    checkouts: BTreeMap<SelectionKey, journal::ActiveCheckoutAdvancedEntry>,
    parents: BTreeMap<ParentKey, WalkPhase>,
}

impl ReplayProjection {
    fn project(&mut self, index: usize, entry: &JournalEntry) -> ReplayStep {
        self.observe(entry);
        replay_step(index, entry, self)
    }

    fn observe(&mut self, entry: &JournalEntry) {
        match entry {
            JournalEntry::ActiveCheckoutAdvanced(entry) => {
                let Some(predecessor) = entry.previous_parent_identity.as_ref() else {
                    return;
                };
                if predecessor.campaign_id() != &entry.campaign_id
                    || entry.selected_parent_identity.campaign_id() != &entry.campaign_id
                {
                    return;
                }
                self.checkouts.insert(
                    (
                        entry.campaign_id.clone(),
                        entry.selected_parent_identity.node_id().to_string(),
                    ),
                    entry.clone(),
                );
            }
            JournalEntry::Successor(record) => {
                let Some(key) = attempt_key(record) else {
                    return;
                };
                match &record.state {
                    crate::cli::prototype1_state::successor::State::Spawned { .. } => {
                        self.spawns.insert(key.clone(), record.clone());
                        let phase = if self.is_acknowledged(&key) {
                            WalkPhase::R13b
                        } else {
                            WalkPhase::R13c
                        };
                        self.mark_parent(&key, phase);
                    }
                    crate::cli::prototype1_state::successor::State::Ready { .. } => {
                        let phase = if self.is_acknowledged(&key) {
                            WalkPhase::R13b
                        } else {
                            WalkPhase::R13c
                        };
                        self.mark_parent(&key, phase);
                    }
                    crate::cli::prototype1_state::successor::State::TimedOut { .. }
                    | crate::cli::prototype1_state::successor::State::ExitedBeforeReady {
                        ..
                    } => {
                        self.failed.insert(key.clone());
                        self.acknowledged.remove(&key);
                        self.mark_parent(&key, WalkPhase::R13c);
                    }
                    crate::cli::prototype1_state::successor::State::Selected { .. }
                    | crate::cli::prototype1_state::successor::State::Stopped { .. }
                    | crate::cli::prototype1_state::successor::State::Checkout { .. }
                    | crate::cli::prototype1_state::successor::State::Completed { .. } => {}
                }
            }
            JournalEntry::SuccessorHandoff(entry) => {
                let key = (
                    entry.campaign_id.clone(),
                    entry.node_id.clone(),
                    entry.runtime_id,
                );
                if !self.failed.contains(&key) && self.spawn_matches_handoff(&key, entry) {
                    self.acknowledged.insert(key.clone(), entry.clone());
                    self.mark_parent(&key, WalkPhase::R13b);
                } else {
                    self.acknowledged.remove(&key);
                    self.mark_parent(&key, WalkPhase::R13c);
                }
            }
            _ => {}
        }
    }

    fn is_acknowledged(&self, key: &AttemptKey) -> bool {
        !self.failed.contains(key)
            && self
                .acknowledged
                .get(key)
                .is_some_and(|entry| self.spawn_matches_handoff(key, entry))
    }

    fn spawn_matches_handoff(
        &self,
        key: &AttemptKey,
        entry: &journal::SuccessorHandoffEntry,
    ) -> bool {
        let Some(spawn) = self.spawns.get(key) else {
            return false;
        };
        let crate::cli::prototype1_state::successor::State::Spawned {
            pid,
            incarnation: _,
            active_parent_root,
            binary_path,
            invocation_path,
            ready_path,
            streams,
        } = &spawn.state
        else {
            return false;
        };
        let selected = (entry.campaign_id.clone(), entry.node_id.clone());
        let Some(checkout) = self.checkouts.get(&selected) else {
            return false;
        };

        spawn.campaign_id == entry.campaign_id
            && spawn.node_id == entry.node_id
            && spawn.runtime_id == Some(entry.runtime_id)
            && key.0 == entry.campaign_id
            && key.1 == entry.node_id
            && key.2 == entry.runtime_id
            && pid == &entry.pid
            && active_parent_root == &entry.active_parent_root
            && binary_path == &entry.binary_path
            && invocation_path == &entry.invocation_path
            && ready_path == &entry.ready_path
            && entry
                .streams
                .as_ref()
                .is_none_or(|handoff_streams| handoff_streams == streams)
            && checkout.campaign_id == entry.campaign_id
            && checkout.selected_parent_identity.campaign_id() == &entry.campaign_id
            && checkout.selected_parent_identity.node_id() == entry.node_id
            && checkout.active_parent_root == entry.active_parent_root
    }

    fn mark_parent(&mut self, key: &AttemptKey, phase: WalkPhase) {
        let selected = (key.0.clone(), key.1.clone());
        let parent = self
            .checkouts
            .get(&selected)
            .and_then(|entry| entry.previous_parent_identity.as_ref())
            .map(parent_key);
        if let Some(parent) = parent {
            self.parents.insert(parent, phase);
        }
    }

    fn handoff_phase(&self, entry: &journal::SuccessorHandoffEntry) -> WalkPhase {
        let key = (
            entry.campaign_id.clone(),
            entry.node_id.clone(),
            entry.runtime_id,
        );
        if self.is_acknowledged(&key) {
            WalkPhase::R13b
        } else {
            WalkPhase::R13c
        }
    }

    fn parent_phase(&self, sample: &journal::resource::Sample) -> WalkPhase {
        match self.parents.get(&sample_parent_key(sample)) {
            Some(WalkPhase::R13b | WalkPhase::R14b) => WalkPhase::R14b,
            Some(WalkPhase::R13c) => WalkPhase::R13c,
            _ => WalkPhase::R14a,
        }
    }
}

fn attempt_key(record: &crate::cli::prototype1_state::successor::Record) -> Option<AttemptKey> {
    record.runtime_id.map(|runtime_id| {
        (
            record.campaign_id.clone(),
            record.node_id.clone(),
            runtime_id,
        )
    })
}

fn parent_key(identity: &crate::cli::prototype1_state::identity::ParentIdentity) -> ParentKey {
    (
        identity.campaign_id().clone(),
        identity.parent_id().to_string(),
        identity.node_id().to_string(),
        identity.generation(),
    )
}

fn sample_parent_key(sample: &journal::resource::Sample) -> ParentKey {
    (
        sample.campaign_id.clone(),
        sample.parent_id.clone(),
        sample.node_id.clone(),
        sample.generation,
    )
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
        let manifest_path = campaign_manifest_path(&campaign_id)?;
        let journal_path = prototype1_transition_journal_path(&manifest_path);
        let journal = PrototypeJournal::new(journal_path.clone());
        let entries = journal.load_entries().map_err(|error| {
            prototype1_state_transition_error("prototype1_replay_journal", error.to_string())
        })?;
        let mut projection = ReplayProjection::default();
        let steps = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| projection.project(index, entry))
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
        let manifest_path = campaign_manifest_path(&self.campaign_id)?;
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
        "successor_handoff" => "parent acknowledged the successor runtime handoff",
        "successor_handoff_mismatch" => {
            "handoff record did not match a spawned, checked-out, nonfailed runtime attempt"
        }
        "successor.checkout" => "successor artifact checkout evidence; parent remains at R12",
        "successor.spawned" => "successor spawned without parent handoff acknowledgement",
        "successor.ready" => "successor reported ready without parent handoff acknowledgement",
        "successor.timed_out" => "successor acknowledgement timed out after parent retirement",
        "successor.exited_before_ready" => {
            "successor exited before acknowledgement after parent retirement"
        }
        "successor.completed" => {
            "successor completion evidence; terminal handoff requires a same-runtime acknowledgement"
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

fn replay_step(index: usize, entry: &JournalEntry, projection: &ReplayProjection) -> ReplayStep {
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
                journal::resource::Phase::ParentComplete => projection.parent_phase(sample),
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
            Some(WalkPhase::R12),
            "active_checkout_advanced".to_string(),
            entry.selected_parent_identity.node_id().to_string(),
            format!(
                "selected_branch={} installed_commit={} active_parent_root={}",
                entry.selected_branch,
                entry.installed_commit,
                entry.active_parent_root.display()
            ),
        ),
        JournalEntry::SuccessorHandoff(entry) => {
            let phase = projection.handoff_phase(entry);
            (
                Some(entry.recorded_at),
                Some(phase),
                if phase == WalkPhase::R13b {
                    "successor_handoff".to_string()
                } else {
                    "successor_handoff_mismatch".to_string()
                },
                entry.node_id.clone(),
                format!(
                    "runtime={} pid={} ready_path={} binary={}",
                    entry.runtime_id,
                    entry.pid,
                    entry.ready_path.display(),
                    entry.binary_path.display()
                ),
            )
        }
        JournalEntry::Successor(record) => successor_step(record, projection),
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
    projection: &ReplayProjection,
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
            Some(WalkPhase::R12),
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
            Some(
                attempt_key(record)
                    .filter(|key| projection.is_acknowledged(key))
                    .map_or(WalkPhase::R13c, |_| WalkPhase::R13b),
            ),
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
            Some(WalkPhase::R12),
            "successor.checkout".to_string(),
            record.node_id.clone(),
            format!(
                "phase={phase:?} branch={} commit={} root={}",
                selected_branch,
                installed_commit.as_deref().unwrap_or("-"),
                active_parent_root.display()
            ),
        ),
        State::Ready {
            pid, ready_path, ..
        } => (
            Some(record.recorded_at),
            Some(
                attempt_key(record)
                    .filter(|key| projection.is_acknowledged(key))
                    .map_or(WalkPhase::R13c, |_| WalkPhase::R13b),
            ),
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
            Some(WalkPhase::R13c),
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
            Some(WalkPhase::R13c),
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
            Some(
                attempt_key(record)
                    .filter(|key| projection.is_acknowledged(key))
                    .map(|_| WalkPhase::R14b)
                    .unwrap_or(WalkPhase::R13c),
            ),
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
    use crate::{
        cli::prototype1_state::{
            identity::{ParentIdentity, write_parent_identity},
            journal::{
                ActiveCheckoutAdvancedEntry, ChildArtifactCommittedEntry, ParentStartedEntry,
                SuccessorHandoffEntry,
            },
            successor::{Record as SuccessorRecord, State as SuccessorState},
        },
        intervention::{Prototype1ContinuationDecision, Prototype1ContinuationDisposition},
    };
    use std::ffi::OsString;
    use uuid::Uuid;

    fn runtime(value: u128) -> RuntimeId {
        RuntimeId(Uuid::from_u128(value))
    }

    fn successor(runtime_id: RuntimeId, state: SuccessorState) -> JournalEntry {
        JournalEntry::Successor(SuccessorRecord {
            runtime_id: Some(runtime_id),
            recorded_at: RecordedAt(10),
            campaign_id: CampaignId::from("campaign-replay-test"),
            node_id: "node-successor".to_string(),
            state,
        })
    }

    fn identity(node_id: &str) -> ParentIdentity {
        ParentIdentity::root_bootstrap(
            CampaignId::from("campaign-replay-test"),
            node_id,
            format!("instance-{node_id}"),
            format!("branch-{node_id}"),
            Some(format!("artifact-{node_id}")),
        )
    }

    fn child_artifact() -> JournalEntry {
        JournalEntry::ChildArtifactCommitted(ChildArtifactCommittedEntry {
            recorded_at: RecordedAt(25),
            campaign_id: CampaignId::from("campaign-replay-test"),
            parent_identity: Some(identity("node-parent")),
            child_identity: identity("node-child"),
            node_id: "node-child".to_string(),
            generation: 1,
            target_relpath: PathBuf::from("src/lib.rs"),
            child_branch: "child-branch".to_string(),
            target_commit: "def456".to_string(),
            identity_commit: None,
        })
    }

    fn checkout() -> JournalEntry {
        JournalEntry::ActiveCheckoutAdvanced(ActiveCheckoutAdvancedEntry {
            recorded_at: RecordedAt(5),
            campaign_id: CampaignId::from("campaign-replay-test"),
            previous_parent_identity: Some(identity("node-parent")),
            selected_parent_identity: identity("node-successor"),
            active_parent_root: PathBuf::from("/tmp/repo"),
            selected_branch: "artifact-successor".to_string(),
            installed_commit: "abc123".to_string(),
        })
    }

    fn spawned(runtime_id: RuntimeId) -> JournalEntry {
        successor(
            runtime_id,
            SuccessorState::Spawned {
                pid: 42,
                incarnation: None,
                active_parent_root: PathBuf::from("/tmp/repo"),
                binary_path: PathBuf::from("/tmp/ploke-eval"),
                invocation_path: PathBuf::from("/tmp/invocation.json"),
                ready_path: PathBuf::from("/tmp/ready.jsonl"),
                streams: journal::Streams {
                    stdout: PathBuf::from("/tmp/stdout"),
                    stderr: PathBuf::from("/tmp/stderr"),
                },
            },
        )
    }

    fn handoff(runtime_id: RuntimeId) -> JournalEntry {
        JournalEntry::SuccessorHandoff(SuccessorHandoffEntry {
            recorded_at: RecordedAt(20),
            campaign_id: CampaignId::from("campaign-replay-test"),
            node_id: "node-successor".to_string(),
            runtime_id,
            active_parent_root: PathBuf::from("/tmp/repo"),
            binary_path: PathBuf::from("/tmp/ploke-eval"),
            invocation_path: PathBuf::from("/tmp/invocation.json"),
            ready_path: PathBuf::from("/tmp/ready.jsonl"),
            streams: None,
            pid: 42,
            acceptance: None,
        })
    }

    fn parent_started(runtime_id: RuntimeId) -> JournalEntry {
        JournalEntry::ParentStarted(ParentStartedEntry {
            recorded_at: RecordedAt(25),
            campaign_id: CampaignId::from("campaign-replay-test"),
            parent_identity: identity("node-successor"),
            repo_root: PathBuf::from("/tmp/repo"),
            handoff_runtime_id: Some(runtime_id),
            pid: 43,
        })
    }

    fn parent_complete() -> JournalEntry {
        let parent = identity("node-parent");
        JournalEntry::Resource(journal::resource::Sample {
            recorded_at: RecordedAt(30),
            campaign_id: CampaignId::from("campaign-replay-test"),
            parent_id: parent.parent_id().to_string(),
            node_id: parent.node_id().to_string(),
            generation: parent.generation(),
            runtime_id: None,
            subject: journal::resource::Subject::CargoTarget,
            phase: journal::resource::Phase::ParentComplete,
            path: PathBuf::from("/tmp/repo/target"),
            status: journal::resource::Status::Measured,
            bytes: Some(1),
            error: None,
        })
    }

    fn load_persisted(entries: Vec<JournalEntry>) -> ReplayCursor {
        let tmp = tempfile::tempdir().expect("tempdir");
        let _guard = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let campaign_id = CampaignId::from("campaign-replay-test");
        let repo_root = tmp.path().join("repo");
        write_parent_identity(&repo_root, &identity("node-parent")).expect("write identity");
        let manifest_path = campaign_manifest_path(&campaign_id).expect("campaign path");
        let mut journal = PrototypeJournal::new(prototype1_transition_journal_path(&manifest_path));
        for entry in entries {
            journal
                .append_with_receipt(entry)
                .expect("persist journal entry");
        }
        ReplayCursor::load(&repo_root).expect("load persisted replay cursor")
    }

    fn project(entries: &[JournalEntry]) -> Vec<ReplayStep> {
        let mut projection = ReplayProjection::default();
        entries
            .iter()
            .enumerate()
            .map(|(index, entry)| projection.project(index, entry))
            .collect()
    }

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

    #[test]
    fn pre_retirement_successor_evidence_remains_r12() {
        let campaign_id = CampaignId::from("campaign-replay-test");
        let parent = identity("node-parent");
        let selected = identity("node-successor");
        let decision = Prototype1ContinuationDecision {
            disposition: Prototype1ContinuationDisposition::ContinueReady,
            selected_next_branch_id: Some("branch-successor".to_string()),
            selected_branch_disposition: None,
            next_generation: 1,
            total_nodes_after_continue: 2,
        };
        let entries = vec![
            JournalEntry::Successor(SuccessorRecord::selected(
                campaign_id.clone(),
                "node-successor".to_string(),
                decision,
            )),
            successor(
                runtime(1),
                SuccessorState::Checkout {
                    phase: crate::intervention::CommitPhase::After,
                    active_parent_root: PathBuf::from("/tmp/repo"),
                    selected_branch: "artifact-successor".to_string(),
                    installed_commit: Some("abc123".to_string()),
                },
            ),
            JournalEntry::ActiveCheckoutAdvanced(ActiveCheckoutAdvancedEntry {
                recorded_at: RecordedAt(5),
                campaign_id,
                previous_parent_identity: Some(parent),
                selected_parent_identity: selected,
                active_parent_root: PathBuf::from("/tmp/repo"),
                selected_branch: "artifact-successor".to_string(),
                installed_commit: "abc123".to_string(),
            }),
        ];

        let phases = project(&entries)
            .into_iter()
            .map(|step| step.phase)
            .collect::<Vec<_>>();

        assert_eq!(phases, vec![Some(WalkPhase::R12); 3]);
    }

    #[test]
    fn unacknowledged_successor_states_project_r13c() {
        let runtime_id = runtime(1);
        let entries = vec![
            successor(
                runtime_id,
                SuccessorState::Spawned {
                    pid: 42,
                    incarnation: None,
                    active_parent_root: PathBuf::from("/tmp/repo"),
                    binary_path: PathBuf::from("/tmp/ploke-eval"),
                    invocation_path: PathBuf::from("/tmp/invocation.json"),
                    ready_path: PathBuf::from("/tmp/ready.jsonl"),
                    streams: journal::Streams {
                        stdout: PathBuf::from("/tmp/stdout"),
                        stderr: PathBuf::from("/tmp/stderr"),
                    },
                },
            ),
            successor(
                runtime_id,
                SuccessorState::Ready {
                    pid: 42,
                    ready_path: PathBuf::from("/tmp/ready.jsonl"),
                    controller: None,
                },
            ),
            successor(
                runtime_id,
                SuccessorState::TimedOut {
                    waited_ms: 10_000,
                    ready_path: PathBuf::from("/tmp/ready.jsonl"),
                },
            ),
            successor(
                runtime_id,
                SuccessorState::ExitedBeforeReady { exit_code: Some(1) },
            ),
        ];

        assert!(
            project(&entries)
                .iter()
                .all(|step| step.phase == Some(WalkPhase::R13c))
        );
    }

    #[test]
    fn completion_requires_same_runtime_handoff() {
        let acknowledged = runtime(1);
        let other = runtime(2);
        let completion = |runtime_id| {
            successor(
                runtime_id,
                SuccessorState::Completed {
                    status: crate::cli::prototype1_state::invocation::SuccessorCompletionStatus::Succeeded,
                    completion_path: PathBuf::from("/tmp/completion.json"),
                    trace_path: None,
                    detail: None,
                },
            )
        };
        let steps = project(&[
            checkout(),
            spawned(acknowledged),
            handoff(acknowledged),
            completion(other),
            completion(acknowledged),
        ]);

        assert_eq!(steps[2].phase, Some(WalkPhase::R13b));
        assert_eq!(steps[3].phase, Some(WalkPhase::R13c));
        assert_eq!(steps[4].phase, Some(WalkPhase::R14b));
    }

    #[test]
    fn parent_complete_does_not_promote_incomplete_handoff() {
        let entries = vec![
            checkout(),
            spawned(runtime(1)),
            successor(
                runtime(1),
                SuccessorState::TimedOut {
                    waited_ms: 10_000,
                    ready_path: PathBuf::from("/tmp/ready.jsonl"),
                },
            ),
            child_artifact(),
            parent_started(runtime(1)),
            parent_complete(),
        ];
        let steps = project(&entries);

        assert_eq!(steps[2].phase, Some(WalkPhase::R13c));
        assert_eq!(steps[3].phase, Some(WalkPhase::R8));
        assert_eq!(steps[4].phase, Some(WalkPhase::R5));
        assert_eq!(steps[5].phase, Some(WalkPhase::R13c));
    }

    #[test]
    fn persisted_replay_keeps_ack_across_successor_start_and_ready() {
        let runtime_id = runtime(1);
        let replay = load_persisted(vec![
            checkout(),
            spawned(runtime_id),
            handoff(runtime_id),
            parent_started(runtime_id),
            successor(
                runtime_id,
                SuccessorState::Ready {
                    pid: 42,
                    ready_path: PathBuf::from("/tmp/ready.jsonl"),
                    controller: None,
                },
            ),
            parent_complete(),
        ]);

        assert_eq!(replay.steps[2].phase, Some(WalkPhase::R13b));
        assert_eq!(replay.steps[3].phase, Some(WalkPhase::R5));
        assert_eq!(replay.steps[4].phase, Some(WalkPhase::R13b));
        assert_eq!(replay.steps[5].phase, Some(WalkPhase::R14b));
    }

    #[test]
    fn persisted_replay_rejects_orphan_and_late_handoff() {
        let runtime_id = runtime(1);
        let orphan = load_persisted(vec![spawned(runtime_id), handoff(runtime_id)]);
        assert_eq!(orphan.steps[1].phase, Some(WalkPhase::R13c));

        let failed = load_persisted(vec![
            checkout(),
            spawned(runtime_id),
            successor(
                runtime_id,
                SuccessorState::TimedOut {
                    waited_ms: 10_000,
                    ready_path: PathBuf::from("/tmp/ready.jsonl"),
                },
            ),
            handoff(runtime_id),
            parent_complete(),
        ]);
        assert_eq!(failed.steps[2].phase, Some(WalkPhase::R13c));
        assert_eq!(failed.steps[3].phase, Some(WalkPhase::R13c));
        assert_eq!(failed.steps[4].phase, Some(WalkPhase::R13c));
    }

    #[test]
    fn persisted_replay_rejects_same_id_handoff_path_and_pid_mismatch() {
        let runtime_id = runtime(1);
        for field in [
            "active_parent_root",
            "binary_path",
            "invocation_path",
            "ready_path",
            "pid",
        ] {
            let mut mismatched = handoff(runtime_id);
            let JournalEntry::SuccessorHandoff(entry) = &mut mismatched else {
                unreachable!("handoff fixture must be a successor handoff")
            };
            match field {
                "active_parent_root" => entry.active_parent_root = PathBuf::from("/tmp/other"),
                "binary_path" => entry.binary_path = PathBuf::from("/tmp/other-ploke-eval"),
                "invocation_path" => entry.invocation_path = PathBuf::from("/tmp/other.json"),
                "ready_path" => entry.ready_path = PathBuf::from("/tmp/other-ready.jsonl"),
                "pid" => entry.pid = 99,
                _ => unreachable!("all mismatch fields are covered"),
            }

            let replay = load_persisted(vec![checkout(), spawned(runtime_id), mismatched.clone()]);
            assert_eq!(
                replay.steps[2].phase,
                Some(WalkPhase::R13c),
                "same-ID {field} mismatch must fail closed"
            );
            assert_eq!(replay.steps[2].kind, "successor_handoff_mismatch");

            let replay = load_persisted(vec![
                checkout(),
                spawned(runtime_id),
                handoff(runtime_id),
                mismatched,
                parent_complete(),
            ]);
            assert_eq!(replay.steps[3].phase, Some(WalkPhase::R13c));
            assert_eq!(replay.steps[4].phase, Some(WalkPhase::R13c));
        }
    }

    #[test]
    fn persisted_replay_rejects_checkout_root_mismatch() {
        let runtime_id = runtime(1);
        let mut mismatched = checkout();
        let JournalEntry::ActiveCheckoutAdvanced(entry) = &mut mismatched else {
            unreachable!("checkout fixture must be an active checkout advance")
        };
        entry.active_parent_root = PathBuf::from("/tmp/other");

        let replay = load_persisted(vec![mismatched, spawned(runtime_id), handoff(runtime_id)]);
        assert_eq!(replay.steps[2].phase, Some(WalkPhase::R13c));
        assert_eq!(replay.steps[2].kind, "successor_handoff_mismatch");
    }

    #[test]
    fn persisted_replay_correlates_present_handoff_streams() {
        let runtime_id = runtime(1);
        let streams = journal::Streams {
            stdout: PathBuf::from("/tmp/stdout"),
            stderr: PathBuf::from("/tmp/stderr"),
        };
        let mut exact = handoff(runtime_id);
        let JournalEntry::SuccessorHandoff(entry) = &mut exact else {
            unreachable!("handoff fixture must be a successor handoff")
        };
        entry.streams = Some(streams.clone());
        let replay = load_persisted(vec![checkout(), spawned(runtime_id), exact]);
        assert_eq!(replay.steps[2].phase, Some(WalkPhase::R13b));

        for field in ["stdout", "stderr"] {
            let mut mismatched = handoff(runtime_id);
            let JournalEntry::SuccessorHandoff(entry) = &mut mismatched else {
                unreachable!("handoff fixture must be a successor handoff")
            };
            let mut handoff_streams = streams.clone();
            match field {
                "stdout" => handoff_streams.stdout = PathBuf::from("/tmp/other-stdout"),
                "stderr" => handoff_streams.stderr = PathBuf::from("/tmp/other-stderr"),
                _ => unreachable!("all stream fields are covered"),
            }
            entry.streams = Some(handoff_streams);

            let replay = load_persisted(vec![checkout(), spawned(runtime_id), mismatched]);
            assert_eq!(
                replay.steps[2].phase,
                Some(WalkPhase::R13c),
                "same-ID {field} mismatch must fail closed"
            );
            assert_eq!(replay.steps[2].kind, "successor_handoff_mismatch");
        }
    }
}
