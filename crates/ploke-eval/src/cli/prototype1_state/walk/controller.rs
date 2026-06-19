//! In-memory typestate walker used by the local debug server.
//!
//! `WalkController` owns exactly one operator-session `WalkState` value. When
//! the server has no state, it first tries durable reconstruction from disk; each
//! live step then consumes the typed state and calls the canonical direct edge
//! functions from `live_edges`. This module should not duplicate transition
//! semantics or become authority for History, parent identity, child terminality,
//! or successor handoff finality.

use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use ploke_records::ids::CampaignId;

use crate::{
    ResolvedCampaignConfig,
    cli::prototype1_state::{
        cli_facing::{Prototype1StateRunShape, campaign_manifest_path_for_id},
        driver::{
            reconstruct::{self, EarlyState},
            replay::ReplayCursor,
        },
        identity::{load_parent_identity_optional, parent_identity_path},
        journal::prototype1_transition_journal_path,
        live_edges::{
            r0_to_r1, r1_to_r2a_or_r3, r3_to_r4a, r4a_to_r4b_or_r4c, r4b_to_r4c_genesis, r4c_to_r5,
            r5_to_r6, r6_to_r7, r7_to_r8, r8_to_r9, r9_to_r10, r10_to_r11, r11_to_r12, r12_to_r13,
            r13_to_r14,
        },
        typestate::{
            self, AsyncStepInput, R0, R1, R2a, R3, R4a, R4bGenesisChecked, R4cReady, R5, R6, R7,
            R8, R9, R10, R11FanoutComplete, R11aRejectedOnly, R12, R13aStopped,
            R13bHandoffCommitted, R14aFinalStopped, R14bFinalHandoff, StepInput,
        },
    },
    layout::prototype1_monitor_target_file,
    replay::tool_loop::{
        FsToolLoopStore, ToolLoopOutcome, ToolLoopResult, ToolLoopResume, ToolLoopSession,
        ToolLoopStatus, ToolLoopStore, WorkspaceState,
    },
    spec::PrepareError,
};

use super::{paths, phase::WalkPhase, protocol::WalkStartConfig};

const MAX_HISTORY: usize = 80;
const MAX_FILE_BYTES: usize = 128 * 1024;

type RunShape = Prototype1StateRunShape;
type CampaignConfig = ResolvedCampaignConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LlmMove {
    Back,
    Forward,
}

/// Single-session in-memory controller for Prototype 1 typestate phases.
///
/// The server admits setup/startup and parent-start phases through live `R7`,
/// watch-gated `R8`, schedule-ready `R9`, strategy-ready `R10`, watch-gated
/// `R11`, report-ready `R12`, guarded handoff-capable `R13`, and final-report
/// `R14`. Parent/successor handoff remains gated by explicit operator admission
/// because it mutates the active checkout and spawns the successor runtime.
pub(crate) struct WalkController {
    repo_root: PathBuf,
    state: WalkState,
    steps: usize,
    previous: WalkPrevious,
    files: WalkFiles,
    last_delta: Option<WalkAdvanceReport>,
    reconstruction: Option<WalkReconstruction>,
    replay: Option<ReplayCursor>,
    llm_focus: Option<String>,
    llm_cursors: BTreeMap<String, usize>,
}

/// Owned typestate value currently held by the server.
///
/// The enum is intentionally private: external callers address state through
/// `WalkPhase`, while only the controller can consume and replace typed values.
enum WalkState {
    Empty,
    R0(R0),
    R1(R1<RunShape, CampaignConfig>),
    R2a(R2a<RunShape, CampaignConfig>),
    R3(R3<RunShape, CampaignConfig>),
    R4a(R4a<RunShape, CampaignConfig>),
    R4b(R4bGenesisChecked<RunShape, CampaignConfig>),
    R4c(R4cReady<RunShape, CampaignConfig>),
    R5(R5<RunShape, CampaignConfig>),
    R6(R6<RunShape, CampaignConfig>),
    R7(R7<RunShape, CampaignConfig>),
    R8(R8<RunShape, CampaignConfig>),
    R9(R9<RunShape, CampaignConfig>),
    R10(R10<RunShape, CampaignConfig>),
    R11a(R11aRejectedOnly<RunShape, CampaignConfig>),
    R11(R11FanoutComplete<RunShape, CampaignConfig>),
    R12(R12<RunShape, CampaignConfig>),
    R13a(R13aStopped<RunShape, CampaignConfig>),
    R13b(R13bHandoffCommitted<RunShape, CampaignConfig>),
    R14a(R14aFinalStopped<RunShape, CampaignConfig>),
    R14b(R14bFinalHandoff<RunShape, CampaignConfig>),
    /// A consuming transition failed after the previous typed value was moved.
    ///
    /// Rust cannot restore the consumed value after an edge returns `Err`, so
    /// the server keeps an inspectable failed cursor and requires a fresh walk.
    Failed {
        phase: WalkPhase,
        detail: String,
    },
}

/// Human-facing summary of one `walk step` request.
#[derive(Clone)]
pub(crate) struct WalkAdvanceReport {
    from: WalkPhase,
    to: WalkPhase,
    transitions: Vec<WalkTransition>,
}

#[derive(Clone, Copy)]
struct WalkTransition {
    from: WalkPhase,
    to: WalkPhase,
}

#[derive(Clone, Copy)]
pub(crate) struct DeltaRenderStyle {
    pub(crate) verbose: bool,
    pub(crate) color: bool,
}

impl WalkAdvanceReport {
    /// Render from/to, applied edges, typestate deltas, and next admitted steps.
    pub(crate) fn render(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("from: {} - {}", self.from, self.from.detail()));
        lines.push(format!("to: {} - {}", self.to, self.to.detail()));
        if self.transitions.is_empty() {
            lines.push("transition: already at requested phase".to_string());
        } else if let [transition] = self.transitions.as_slice() {
            lines.push(format!("edge: {}", transition.edge()));
            push_changes(
                &mut lines,
                transition.from,
                transition.to,
                "typestate changes",
            );
        } else {
            lines.push("transitions:".to_string());
            for (index, transition) in self.transitions.iter().enumerate() {
                lines.push(format!(
                    "  {}. {} -> {} via {}",
                    index + 1,
                    transition.from,
                    transition.to,
                    transition.edge()
                ));
            }
            if let Some(last) = self.transitions.last() {
                push_changes(&mut lines, last.from, last.to, "last typestate changes");
            }
        }
        push_next_steps(&mut lines, self.to);
        lines.join("\n")
    }

    /// Render only the delta from this step request.
    pub(crate) fn render_delta(&self, style: DeltaRenderStyle) -> String {
        let mut lines = Vec::new();
        lines.push(format!("from: {} - {}", self.from, self.from.detail()));
        lines.push(format!("to: {} - {}", self.to, self.to.detail()));
        if self.transitions.is_empty() {
            lines.push("transition: already at requested phase".to_string());
            return lines.join("\n");
        }
        if let [transition] = self.transitions.as_slice() {
            transition.push_delta(&mut lines, style);
        } else {
            lines.push("transitions:".to_string());
            for (index, transition) in self.transitions.iter().enumerate() {
                lines.push(format!(
                    "  {}. {} -> {} via {}",
                    index + 1,
                    transition.from,
                    transition.to,
                    transition.edge()
                ));
            }
            if let Some(last) = self.transitions.last() {
                lines.push("last transition delta:".to_string());
                last.push_delta(&mut lines, style);
            }
        }
        lines.join("\n")
    }
}

impl WalkTransition {
    fn edge(self) -> &'static str {
        self.to.edge_from(self.from).unwrap_or("unknown_edge")
    }

    fn push_delta(self, lines: &mut Vec<String>, style: DeltaRenderStyle) {
        lines.push(format!(
            "edge: {}",
            highlight_changed(self.edge(), style.color)
        ));
        if style.verbose {
            push_verbose_changes(lines, self.from, self.to, style);
        } else {
            push_colored_changes(lines, self.from, self.to, style);
        }
    }
}

impl WalkController {
    /// Create an empty controller with no active walk.
    pub(crate) fn new(repo_root: PathBuf) -> Self {
        let mut files = WalkFiles::default();
        files.reset(&repo_root);
        Self {
            repo_root,
            state: WalkState::Empty,
            steps: 0,
            previous: WalkPrevious::default(),
            files,
            last_delta: None,
            reconstruction: None,
            replay: None,
            llm_focus: None,
            llm_cursors: BTreeMap::new(),
        }
    }

    /// Return the current protocol-visible phase cursor.
    pub(crate) fn phase(&self) -> WalkPhase {
        self.state.phase()
    }

    /// Produce a human-readable summary for `show` and `health`.
    pub(crate) fn describe(&self) -> String {
        let phase = self.phase();
        let mut lines = Vec::new();
        match &self.state {
            WalkState::Failed { phase, detail } => lines.push(format!(
                "walk failed while advancing from {phase}; steps={}; detail={detail}",
                self.steps
            )),
            _ => lines.push(format!("phase: {phase} - {}", phase.detail())),
        }
        lines.push(format!("steps: {}", self.steps));
        self.files.push_roots(&mut lines);
        self.files.push_tracked(&mut lines);
        if let Some(reconstruction) = &self.reconstruction {
            reconstruction.push_lines(&mut lines);
        }
        lines.push("typestate:".to_string());
        let typestate = phase.typestate();
        lines.extend(indent_lines(&typestate, 2));
        push_next_steps(&mut lines, phase);
        lines.push("previous:".to_string());
        lines.extend(self.previous.lines());
        lines.join("\n")
    }

    /// Render tracked output files for the current walk.
    pub(crate) fn files_report(&self) -> String {
        self.files.render()
    }

    /// Render known nested LLM/tool-loop fanout lanes without mutating the walk.
    pub(crate) fn llm_lanes_report(&self, verbose: bool) -> Result<String, PrepareError> {
        let store = self.tool_loop_store()?;
        let lanes = self.llm_lanes(&store)?;
        let mut lines = Vec::new();
        lines.push("llm fanout lanes".to_string());
        lines.push(format!("root: {}", store.root().display()));
        lines.push(format!(
            "focus: {}",
            self.llm_focus.as_deref().unwrap_or("-")
        ));
        if lanes.is_empty() {
            lines.push("lanes: (none)".to_string());
            lines.push(
                "hint: run a live headless harness/fanout edge with response capture first"
                    .to_string(),
            );
            return Ok(lines.join("\n"));
        }
        lines.push("lanes:".to_string());
        for lane in lanes {
            let marker = if self.llm_focus.as_deref() == Some(lane.lane_id.as_str()) {
                "*"
            } else {
                " "
            };
            lines.push(format!(
                "{marker} {} status={} cursor={} head={} next_step={} model={}",
                lane.lane_id,
                status_label(lane.session.status),
                self.llm_cursors
                    .get(&lane.lane_id)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                lane.head
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                lane.resume
                    .as_ref()
                    .map(|resume| resume.next_step.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                lane.session.model.as_deref().unwrap_or("-")
            ));
            if verbose {
                lines.push(format!("    session: {}", lane.session.session_id));
                lines.push(format!(
                    "    workspace: {}",
                    lane.session.workspace.display()
                ));
            }
        }
        lines.push("hint: use `walk llm focus <lane>` then `walk llm show --head`".to_string());
        Ok(lines.join("\n"))
    }

    /// Set the default LLM/tool-loop lane focus for this walk server.
    pub(crate) fn llm_focus(&mut self, lane: String) -> Result<String, PrepareError> {
        let store = self.tool_loop_store()?;
        let lane_state = self.resolve_lane(&store, Some(lane.as_str()))?;
        self.llm_focus = Some(lane_state.lane_id.clone());
        if let Some(head) = lane_state.head {
            self.llm_cursors
                .entry(lane_state.lane_id.clone())
                .or_insert(head);
        }
        Ok(format!(
            "focused llm lane {} (session {} head={})",
            lane_state.lane_id,
            lane_state.session.session_id,
            lane_state
                .head
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string())
        ))
    }

    /// Render nested LLM/tool-loop checkpoint state without mutating the walk.
    pub(crate) fn llm_report(
        &self,
        session_id: Option<&str>,
        lane: Option<&str>,
        head: bool,
        step: Option<usize>,
    ) -> Result<String, PrepareError> {
        let store = self.tool_loop_store()?;
        let lane_state = match session_id {
            Some(session_id) => self.lane_for_session(&store, store.read_session(session_id)?)?,
            None => self.resolve_lane(&store, lane)?,
        };
        let selected = match step {
            Some(step) => Some(step),
            None if head => lane_state.head,
            None => self
                .llm_cursors
                .get(&lane_state.lane_id)
                .copied()
                .or(lane_state.head),
        };
        let loaded = selected
            .map(|step| {
                store
                    .read_step(&lane_state.session.session_id, step)
                    .map(|record| (step, record))
            })
            .transpose()?;

        Ok(self.render_llm_checkpoint(&store, lane_state, loaded))
    }

    /// Move a read-only lane cursor backward/forward without mutating durable state.
    pub(crate) fn llm_move(
        &mut self,
        lane: Option<&str>,
        steps: usize,
        direction: LlmMove,
    ) -> Result<String, PrepareError> {
        let store = self.tool_loop_store()?;
        let lane_state = self.resolve_lane(&store, lane)?;
        let head = lane_state
            .head
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "llm lane '{}' has no recorded response steps to move through",
                    lane_state.lane_id
                ),
            })?;
        let current = self
            .llm_cursors
            .get(&lane_state.lane_id)
            .copied()
            .unwrap_or(head);
        let next = match direction {
            LlmMove::Back => current.saturating_sub(steps),
            LlmMove::Forward => current.saturating_add(steps).min(head),
        };
        self.llm_cursors.insert(lane_state.lane_id.clone(), next);
        Ok(format!(
            "llm lane {} cursor={} head={}",
            lane_state.lane_id, next, head
        ))
    }

    /// Move a read-only lane cursor to the latest checkpoint head.
    pub(crate) fn llm_head(&mut self, lane: Option<&str>) -> Result<String, PrepareError> {
        let store = self.tool_loop_store()?;
        let lane_state = self.resolve_lane(&store, lane)?;
        let head = lane_state
            .head
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "llm lane '{}' has no recorded response steps to jump to",
                    lane_state.lane_id
                ),
            })?;
        self.llm_cursors.insert(lane_state.lane_id.clone(), head);
        Ok(format!(
            "llm lane {} cursor=head ({head})",
            lane_state.lane_id
        ))
    }

    fn tool_loop_store(&self) -> Result<FsToolLoopStore, PrepareError> {
        let identity = load_parent_identity_optional(&self.repo_root)?.ok_or_else(|| {
            PrepareError::DatabaseSetup {
                phase: "walk_llm_store",
                detail: format!(
                    "parent identity is required to locate campaign tool-loop checkpoints: {}",
                    parent_identity_path(&self.repo_root).display()
                ),
            }
        })?;
        let manifest = campaign_manifest_path_for_id(identity.campaign_id())?;
        let campaign_dir = manifest
            .parent()
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "walk_llm_store",
                detail: format!(
                    "campaign manifest '{}' has no parent directory",
                    manifest.display()
                ),
            })?;
        Ok(FsToolLoopStore::new(
            campaign_dir.join("prototype1/debug/tool-loop"),
        ))
    }

    fn llm_lanes(&self, store: &FsToolLoopStore) -> Result<Vec<LlmLane>, PrepareError> {
        let mut by_lane = BTreeMap::<String, LlmLane>::new();
        for session in store.list_sessions()? {
            let lane = self.lane_for_session(store, session)?;
            by_lane
                .entry(lane.lane_id.clone())
                .and_modify(|current| {
                    if lane_sort_key(&lane) >= lane_sort_key(current) {
                        *current = lane.clone();
                    }
                })
                .or_insert(lane);
        }
        Ok(by_lane.into_values().collect())
    }

    fn resolve_lane(
        &self,
        store: &FsToolLoopStore,
        requested: Option<&str>,
    ) -> Result<LlmLane, PrepareError> {
        let lanes = self.llm_lanes(store)?;
        if lanes.is_empty() {
            return Err(PrepareError::DatabaseSetup {
                phase: "walk_llm_lane",
                detail: format!(
                    "no tool-loop checkpoint sessions found under '{}'",
                    store.root().display()
                ),
            });
        }
        let target = requested.or(self.llm_focus.as_deref());
        if let Some(target) = target {
            return lanes
                .into_iter()
                .find(|lane| lane.lane_id == target || lane.session.session_id == target)
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "unknown llm lane '{target}'; run `walk llm lanes` to list available lanes"
                    ),
                });
        }
        lanes
            .into_iter()
            .max_by_key(lane_sort_key)
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "walk_llm_lane",
                detail: "no llm lanes available".to_string(),
            })
    }

    fn lane_for_session(
        &self,
        store: &FsToolLoopStore,
        session: ToolLoopSession,
    ) -> Result<LlmLane, PrepareError> {
        let lane_id = lane_label(&session);
        let resume = store.read_resume(&session.session_id).ok();
        let head = store.latest_step_index(&session.session_id)?;
        Ok(LlmLane {
            lane_id,
            session,
            resume,
            head,
        })
    }

    fn render_llm_checkpoint(
        &self,
        store: &FsToolLoopStore,
        lane: LlmLane,
        loaded: Option<(usize, crate::replay::tool_loop::ToolLoopStep)>,
    ) -> String {
        let mut lines = Vec::new();
        lines.push("llm tool-loop checkpoint".to_string());
        lines.push(format!("root: {}", store.root().display()));
        lines.push(format!("lane: {}", lane.lane_id));
        lines.push(format!("session: {}", lane.session.session_id));
        lines.push(format!("status: {}", status_label(lane.session.status)));
        lines.push(format!("harness: {}", lane.session.harness));
        lines.push(format!("workspace: {}", lane.session.workspace.display()));
        if let Some(model) = lane.session.model.as_deref() {
            lines.push(format!("model: {model}"));
        }
        if let Some(fanout) = lane.session.fanout_id.as_deref() {
            lines.push(format!("fanout_id: {fanout}"));
        }
        if let Some(phase) = lane.session.outer_phase.as_deref() {
            lines.push(format!("outer_phase: {phase}"));
        }
        if let Some(edge) = lane.session.outer_edge.as_deref() {
            lines.push(format!("outer_edge: {edge}"));
        }
        lines.push(format!(
            "cursor: {}",
            self.llm_cursors
                .get(&lane.lane_id)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string())
        ));
        lines.push(format!(
            "head: {}",
            lane.head
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string())
        ));
        match lane.resume {
            Some(resume) => {
                lines.push(format!("next_step: {}", resume.next_step));
                lines.push(format!(
                    "resume_messages: {}",
                    resume.request_messages.len()
                ));
                lines.push(format!("resume_terminal: {}", resume.terminal));
            }
            None => lines.push("resume: (missing)".to_string()),
        }
        match loaded {
            Some((step, record)) => {
                lines.push(format!("step: {step}"));
                lines.push(format!(
                    "response_index: {}",
                    record.response.response_index().get()
                ));
                lines.push(format!("outcome: {}", outcome_label(&record.outcome)));
                lines.push(format!("tool_requests: {}", record.tool_requests.len()));
                lines.push(format!("tool_results: {}", record.tool_results.len()));
                for result in record.tool_results.iter().take(6) {
                    lines.push(format!("  {}", result_label(result)));
                }
                if record.tool_results.len() > 6 {
                    lines.push(format!(
                        "  ... {} more result(s)",
                        record.tool_results.len() - 6
                    ));
                }
                lines.push(format!("terminal: {}", record.terminal));
                lines.push(format!(
                    "workspace_before: {}",
                    workspace_label(&record.workspace_before)
                ));
                lines.push(format!(
                    "workspace_after: {}",
                    workspace_label(&record.workspace_after)
                ));
            }
            None => lines.push("step: (none recorded yet)".to_string()),
        }
        lines.join("\n")
    }

    /// Render the last successful step delta, if any.
    pub(crate) fn delta_report(&self, style: DeltaRenderStyle) -> String {
        self.last_delta
            .as_ref()
            .map(|delta| delta.render_delta(style))
            .unwrap_or_else(|| "no previous step delta; run `walk step` first".to_string())
    }

    /// Render or position the read-only historical replay cursor.
    pub(crate) fn replay_report(
        &mut self,
        index: Option<usize>,
        tail: usize,
    ) -> Result<String, PrepareError> {
        let cursor = self.replay_cursor_mut()?;
        if let Some(index) = index {
            cursor.jump(index)?;
        }
        Ok(cursor.render(tail))
    }

    /// Move the read-only replay cursor backward without mutating durable state.
    pub(crate) fn replay_back(
        &mut self,
        steps: usize,
        tail: usize,
    ) -> Result<String, PrepareError> {
        let cursor = self.replay_cursor_mut()?;
        cursor.back(steps);
        Ok(cursor.render(tail))
    }

    /// Move the read-only replay cursor forward without mutating durable state.
    pub(crate) fn replay_forward(
        &mut self,
        steps: usize,
        tail: usize,
    ) -> Result<String, PrepareError> {
        let cursor = self.replay_cursor_mut()?;
        cursor.forward(steps);
        Ok(cursor.render(tail))
    }

    /// Record explicit provenance before a future replay-to-live branch.
    pub(crate) fn record_replay_branch(&mut self, reason: String) -> Result<String, PrepareError> {
        let cursor = self.replay_cursor_mut()?;
        let path = cursor.write_branch_provenance(reason)?;
        Ok(format!(
            "replay-to-live provenance recorded at {}\nno live worktree or provider call was started by this command",
            path.display()
        ))
    }

    /// Reset the current in-memory walk without stopping the server process.
    pub(crate) fn reset(&mut self) -> WalkPhase {
        let previous = self.phase();
        self.state = WalkState::Empty;
        self.steps = 0;
        self.files.reset(&self.repo_root);
        self.last_delta = None;
        self.reconstruction = None;
        self.replay = None;
        self.llm_focus = None;
        self.llm_cursors.clear();
        self.record(format!("reset: cleared in-memory walk from {previous}"));
        self.phase()
    }

    /// Start a new walk from `R0` and optionally advance to an admitted boundary.
    pub(crate) async fn start(
        &mut self,
        config: WalkStartConfig,
        until: WalkPhase,
    ) -> Result<WalkPhase, PrepareError> {
        if !matches!(self.state, WalkState::Empty | WalkState::Failed { .. }) {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "walk is already started at {}; run reset or stop and restart the server to begin a new walk",
                    self.phase()
                ),
            });
        }
        ensure_supported_target(until)?;
        let repo_root = paths::resolve_repo_root(config.repo_root.as_deref())?;
        let init_parent_identity = config.init_parent_identity;
        let requested_campaign = config.campaign.clone();
        self.repo_root = repo_root.clone();
        self.files.reset(&repo_root);
        self.steps = 0;
        self.last_delta = None;
        self.reconstruction = None;
        self.replay = None;
        self.llm_focus = None;
        self.llm_cursors.clear();

        let reconstruct_matches_request = match requested_campaign.as_ref() {
            Some(campaign_id) => load_parent_identity_optional(&repo_root)?
                .is_some_and(|identity| identity.campaign_id() == campaign_id),
            None => true,
        };
        if until != WalkPhase::R0 && !init_parent_identity && reconstruct_matches_request {
            self.refresh_from_disk()?;
            let reconstructed = self.phase();
            if reconstructed != WalkPhase::Empty {
                if phase_rank(reconstructed) < phase_rank(until) {
                    self.advance_until(until, false, false).await?;
                }
                return Ok(self.phase());
            }
            self.state = WalkState::Empty;
            self.reconstruction = None;
        }

        self.state = WalkState::R0(typestate::R0::new(config.into_state_command()));
        self.record(format!(
            "start: created r0 for repo_root '{}'",
            repo_root.display()
        ));
        self.advance_until(until, false, false).await?;
        Ok(self.phase())
    }

    /// Advance the current walk by one edge or until a requested phase.
    pub(crate) async fn step(
        &mut self,
        until: Option<WalkPhase>,
        watch: bool,
        allow_git_changes: bool,
    ) -> Result<WalkAdvanceReport, PrepareError> {
        self.refresh_from_disk()?;
        let from = self.phase();
        let branch_step = until.is_none() && watch && self.phase() == WalkPhase::R10;
        let r12_selected =
            matches!(&self.state, WalkState::R12(r12) if r12.has_successor_selection());
        let target = until.unwrap_or_else(|| {
            if watch && self.phase() == WalkPhase::R7 {
                WalkPhase::R8
            } else if r12_selected {
                WalkPhase::R13b
            } else {
                self.phase().next().unwrap_or(self.phase())
            }
        });
        ensure_supported_target(target)?;
        self.ensure_branch_target(target)?;
        let transitions = if self.phase() == target && !branch_step {
            Vec::new()
        } else if until.is_some() {
            self.advance_until(target, watch, allow_git_changes).await?
        } else {
            vec![self.step_once(watch, allow_git_changes).await?]
        };
        let report = WalkAdvanceReport {
            from,
            to: self.phase(),
            transitions,
        };
        self.last_delta = Some(report.clone());
        Ok(report)
    }

    fn ensure_branch_target(&self, target: WalkPhase) -> Result<(), PrepareError> {
        if let WalkState::R12(r12) = &self.state {
            let selected = r12.has_successor_selection();
            if selected && matches!(target, WalkPhase::R13a | WalkPhase::R14a) {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "target {target} is the stopped/no-selection branch, but R12 has selected-successor evidence; use --until r13b or --until r14b with --watch --allow git-changes"
                    ),
                });
            }
            if !selected && matches!(target, WalkPhase::R13b | WalkPhase::R14b) {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "target {target} is the successor-handoff branch, but R12 has no selected-successor evidence; use --until r13a or --until r14a"
                    ),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn refresh_from_disk(&mut self) -> Result<(), PrepareError> {
        if !matches!(self.state, WalkState::Empty) {
            return Ok(());
        }
        let snapshot = reconstruct::reconstruct_early(&self.repo_root)?;
        let reconstruct::EarlySnapshot {
            state,
            campaign_id,
            notes,
            blockers,
        } = snapshot;
        if let Some(campaign_id) = &campaign_id {
            self.files.remember_campaign(campaign_id);
        }
        self.reconstruction = Some(WalkReconstruction { notes, blockers });
        if let Some(state) = state {
            let phase = phase_for_early(&state);
            self.state = WalkState::from_early(state);
            self.record(format!("reconstructed durable walk state at {phase}"));
        }
        Ok(())
    }

    async fn advance_until(
        &mut self,
        target: WalkPhase,
        watch: bool,
        allow_git_changes: bool,
    ) -> Result<Vec<WalkTransition>, PrepareError> {
        let mut guard = 0_u8;
        let mut transitions = Vec::new();
        while self.phase() != target {
            guard += 1;
            if guard > 16 {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!("walk exceeded early-step guard while advancing to {target}"),
                });
            }
            transitions.push(self.step_once(watch, allow_git_changes).await?);
            if phase_rank(self.phase()) > phase_rank(target)
                || (phase_rank(self.phase()) == phase_rank(target) && self.phase() != target)
            {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "walk reached branch {} while advancing to {target}; rerun with the matching --until target",
                        self.phase()
                    ),
                });
            }
        }
        Ok(transitions)
    }

    async fn step_once(
        &mut self,
        watch: bool,
        allow_git_changes: bool,
    ) -> Result<WalkTransition, PrepareError> {
        let state = std::mem::replace(&mut self.state, WalkState::Empty);
        let previous = state.phase();
        let next = match state {
            WalkState::Empty => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: "walk has not been started; run start first".to_string(),
                });
            }
            WalkState::Failed { phase, detail } => {
                self.state = WalkState::Failed { phase, detail };
                return Err(PrepareError::InvalidBatchSelection {
                    detail: "walk is failed; start a new walk to continue".to_string(),
                });
            }
            WalkState::R0(r0) => match r0.advance(r0_to_r1) {
                Ok(r1) => {
                    self.files.remember_campaign(r1.campaign_id());
                    Ok(WalkState::R1(r1))
                }
                Err(error) => Err(error),
            },
            WalkState::R1(r1) => r1.advance(r1_to_r2a_or_r3).map(|branch| match branch {
                typestate::R1Branch::R2a(r2a) => WalkState::R2a(r2a),
                typestate::R1Branch::R3(r3) => WalkState::R3(r3),
            }),
            WalkState::R2a(r2a) => {
                self.state = WalkState::R2a(r2a);
                let detail = "walk reached R2a parent-identity initialization boundary; no next admitted step is defined";
                self.record(format!("blocked at {previous}: {detail}"));
                return Err(PrepareError::InvalidBatchSelection {
                    detail: detail.to_string(),
                });
            }
            WalkState::R3(r3) => r3.advance(r3_to_r4a).map(WalkState::R4a),
            WalkState::R4a(r4a) => r4a.advance(r4a_to_r4b_or_r4c).map(|branch| match branch {
                typestate::R4aStartupBranch::GenesisChecked(r4b) => WalkState::R4b(r4b),
                typestate::R4aStartupBranch::PredecessorReady(r4c) => WalkState::R4c(r4c),
            }),
            WalkState::R4b(r4b) => r4b.advance(r4b_to_r4c_genesis).map(WalkState::R4c),
            WalkState::R4c(r4c) => r4c.advance(r4c_to_r5).map(WalkState::R5),
            WalkState::R5(r5) => r5.advance_async(r5_to_r6).await.map(WalkState::R6),
            WalkState::R6(r6) => r6.advance(r6_to_r7).map(WalkState::R7),
            WalkState::R7(r7) => {
                if watch {
                    r7.advance_async(r7_to_r8).await.map(WalkState::R8)
                } else {
                    self.state = WalkState::R7(r7);
                    let detail = "walk reached R7 policy-ready boundary; rerun `walk step --watch` to admit the live R8 child-plan authority edge";
                    self.record(format!("blocked at {previous}: {detail}"));
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: detail.to_string(),
                    });
                }
            }
            WalkState::R8(r8) => r8.advance(r8_to_r9).map(WalkState::R9),
            WalkState::R9(r9) => r9.advance(r9_to_r10).map(WalkState::R10),
            WalkState::R10(r10) => {
                if watch {
                    r10.advance_async(r10_to_r11)
                        .await
                        .map(|branch| match branch {
                            typestate::R10FanoutBranch::RejectedOnly(r11a) => WalkState::R11a(r11a),
                            typestate::R10FanoutBranch::FanoutComplete(r11) => WalkState::R11(r11),
                        })
                } else {
                    self.state = WalkState::R10(r10);
                    let detail = "walk reached R10 selection-strategy boundary; rerun `walk step --watch` to admit the live R11 rejected-only/fanout edge";
                    self.record(format!("blocked at {previous}: {detail}"));
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: detail.to_string(),
                    });
                }
            }
            WalkState::R11a(r11a) => {
                r11_to_r12(typestate::R10FanoutBranch::RejectedOnly(r11a)).map(WalkState::R12)
            }
            WalkState::R11(r11) => {
                r11_to_r12(typestate::R10FanoutBranch::FanoutComplete(r11)).map(WalkState::R12)
            }
            WalkState::R12(r12) => {
                // A selected successor may be a rejected child when traversal
                // policy admits `explore_from_rejected`; the selected coordinate
                // is still the handoff target. The extra gates here are about
                // handoff side effects: active checkout mutation, History seal,
                // parent retirement, and successor spawn/ready evidence.
                // See docs/active/agents/2026-06-17_typestate-loop-driver-plan.md
                // Slice 10 and
                // docs/workflow/evalnomicon/src/prototype1/selection-and-evaluation.md.
                if r12.has_successor_selection() && !watch {
                    self.state = WalkState::R12(r12);
                    let detail = "walk reached R12 with selected-successor evidence; rerun `walk step --watch --allow git-changes` to admit R13b handoff";
                    self.record(format!("blocked at {previous}: {detail}"));
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: detail.to_string(),
                    });
                }
                if r12.has_successor_selection() && !allow_git_changes {
                    self.state = WalkState::R12(r12);
                    let detail = "walk R13b handoff installs the selected successor into the active checkout; rerun with `--allow git-changes`";
                    self.record(format!("blocked at {previous}: {detail}"));
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: detail.to_string(),
                    });
                }
                r12.advance(r12_to_r13).map(|branch| match branch {
                    typestate::R12ContinuationBranch::Stopped(r13a) => WalkState::R13a(r13a),
                    typestate::R12ContinuationBranch::HandoffCommitted(r13b) => {
                        WalkState::R13b(r13b)
                    }
                })
            }
            WalkState::R13a(r13a) => r13_to_r14(typestate::R12ContinuationBranch::Stopped(r13a))
                .map(|branch| match branch {
                    typestate::R14FinalBranch::Stopped(r14a) => WalkState::R14a(r14a),
                    typestate::R14FinalBranch::Handoff(_) => {
                        unreachable!("R13a stopped branch cannot produce handoff final state")
                    }
                }),
            WalkState::R13b(r13b) => {
                r13_to_r14(typestate::R12ContinuationBranch::HandoffCommitted(r13b)).map(|branch| {
                    match branch {
                        typestate::R14FinalBranch::Stopped(_) => {
                            unreachable!("R13b handoff branch cannot produce stopped final state")
                        }
                        typestate::R14FinalBranch::Handoff(r14b) => WalkState::R14b(r14b),
                    }
                })
            }
            WalkState::R14a(r14a) => {
                self.state = WalkState::R14a(r14a);
                let detail = "walk reached R14a final stopped-report boundary";
                self.record(format!("blocked at {previous}: {detail}"));
                return Err(PrepareError::InvalidBatchSelection {
                    detail: detail.to_string(),
                });
            }
            WalkState::R14b(r14b) => {
                self.state = WalkState::R14b(r14b);
                let detail = "walk reached R14b final successor-handoff report boundary";
                self.record(format!("blocked at {previous}: {detail}"));
                return Err(PrepareError::InvalidBatchSelection {
                    detail: detail.to_string(),
                });
            }
        };
        match next {
            Ok(state) => {
                let current = state.phase();
                self.state = state;
                self.steps += 1;
                self.reconstruction = None;
                self.record(format!("step {}: {previous} -> {current}", self.steps));
                Ok(WalkTransition {
                    from: previous,
                    to: current,
                })
            }
            Err(error) => {
                let error = if previous == WalkPhase::R4a {
                    PrepareError::DatabaseSetup {
                        phase: "prototype1_parent_checkout",
                        detail: reconstruct::format_r4a_blocker(&self.repo_root, &error),
                    }
                } else {
                    error
                };
                let detail = error.to_string();
                self.state = WalkState::Failed {
                    phase: previous,
                    detail: detail.clone(),
                };
                self.record(format!("failed at {previous}: {detail}"));
                Err(error)
            }
        }
    }

    fn replay_cursor_mut(&mut self) -> Result<&mut ReplayCursor, PrepareError> {
        if self.replay.is_none() {
            self.replay = Some(ReplayCursor::load(&self.repo_root)?);
        }
        Ok(self.replay.as_mut().expect("replay cursor was just loaded"))
    }

    fn record(&mut self, entry: impl Into<String>) {
        self.previous.push(entry.into());
    }
}

impl WalkState {
    fn phase(&self) -> WalkPhase {
        match self {
            WalkState::Empty => WalkPhase::Empty,
            WalkState::R0(_) => WalkPhase::R0,
            WalkState::R1(_) => WalkPhase::R1,
            WalkState::R2a(_) => WalkPhase::R2a,
            WalkState::R3(_) => WalkPhase::R3,
            WalkState::R4a(_) => WalkPhase::R4a,
            WalkState::R4b(_) => WalkPhase::R4b,
            WalkState::R4c(_) => WalkPhase::R4c,
            WalkState::R5(_) => WalkPhase::R5,
            WalkState::R6(_) => WalkPhase::R6,
            WalkState::R7(_) => WalkPhase::R7,
            WalkState::R8(_) => WalkPhase::R8,
            WalkState::R9(_) => WalkPhase::R9,
            WalkState::R10(_) => WalkPhase::R10,
            WalkState::R11a(_) => WalkPhase::R11a,
            WalkState::R11(_) => WalkPhase::R11,
            WalkState::R12(_) => WalkPhase::R12,
            WalkState::R13a(_) => WalkPhase::R13a,
            WalkState::R13b(_) => WalkPhase::R13b,
            WalkState::R14a(_) => WalkPhase::R14a,
            WalkState::R14b(_) => WalkPhase::R14b,
            WalkState::Failed { phase, .. } => *phase,
        }
    }

    fn from_early(state: EarlyState) -> Self {
        match state {
            EarlyState::R1(r1) => WalkState::R1(r1),
            EarlyState::R3(r3) => WalkState::R3(r3),
            EarlyState::R4a(r4a) => WalkState::R4a(r4a),
            EarlyState::R4b(r4b) => WalkState::R4b(r4b),
            EarlyState::R4c(r4c) => WalkState::R4c(r4c),
            EarlyState::R5(r5) => WalkState::R5(r5),
            EarlyState::R6(r6) => WalkState::R6(r6),
            EarlyState::R7(r7) => WalkState::R7(r7),
            EarlyState::R10(r10) => WalkState::R10(r10),
            EarlyState::R12(r12) => WalkState::R12(r12),
            EarlyState::R13a(r13a) => WalkState::R13a(r13a),
            EarlyState::R13b(r13b) => WalkState::R13b(r13b),
            EarlyState::R14a(r14a) => WalkState::R14a(r14a),
            EarlyState::R14b(r14b) => WalkState::R14b(r14b),
        }
    }
}

fn phase_for_early(state: &EarlyState) -> WalkPhase {
    match state {
        EarlyState::R1(_) => WalkPhase::R1,
        EarlyState::R3(_) => WalkPhase::R3,
        EarlyState::R4a(_) => WalkPhase::R4a,
        EarlyState::R4b(_) => WalkPhase::R4b,
        EarlyState::R4c(_) => WalkPhase::R4c,
        EarlyState::R5(_) => WalkPhase::R5,
        EarlyState::R6(_) => WalkPhase::R6,
        EarlyState::R7(_) => WalkPhase::R7,
        EarlyState::R10(_) => WalkPhase::R10,
        EarlyState::R12(_) => WalkPhase::R12,
        EarlyState::R13a(_) => WalkPhase::R13a,
        EarlyState::R13b(_) => WalkPhase::R13b,
        EarlyState::R14a(_) => WalkPhase::R14a,
        EarlyState::R14b(_) => WalkPhase::R14b,
    }
}

struct WalkReconstruction {
    notes: Vec<String>,
    blockers: Vec<String>,
}

impl WalkReconstruction {
    fn push_lines(&self, lines: &mut Vec<String>) {
        lines.push("reconstruction:".to_string());
        if self.notes.is_empty() && self.blockers.is_empty() {
            lines.push("  (no durable reconstruction notes)".to_string());
            return;
        }
        for note in &self.notes {
            push_wrapped_item(lines, note, "  - ", "    ");
        }
        for blocker in &self.blockers {
            push_wrapped_item(lines, blocker, "  - blocked: ", "    ");
        }
    }
}

fn push_wrapped_item(lines: &mut Vec<String>, value: &str, first_prefix: &str, rest_prefix: &str) {
    let mut item = value.lines();
    if let Some(first) = item.next() {
        lines.push(format!("{first_prefix}{first}"));
        for line in item {
            lines.push(format!("{rest_prefix}{line}"));
        }
    }
}

fn phase_rank(phase: WalkPhase) -> u8 {
    match phase {
        WalkPhase::Empty => 0,
        WalkPhase::R0 => 1,
        WalkPhase::R1 => 2,
        WalkPhase::R2a | WalkPhase::R3 => 3,
        WalkPhase::R4a => 4,
        WalkPhase::R4b | WalkPhase::R4c => 5,
        WalkPhase::R5 => 6,
        WalkPhase::R6 => 7,
        WalkPhase::R7 => 8,
        WalkPhase::R8 => 9,
        WalkPhase::R9 => 10,
        WalkPhase::R10 => 11,
        WalkPhase::R11a | WalkPhase::R11 => 12,
        WalkPhase::R12 => 13,
        WalkPhase::R13a | WalkPhase::R13b => 14,
        WalkPhase::R14a | WalkPhase::R14b => 15,
    }
}

trait NextPhase {
    fn next(self) -> Option<WalkPhase>;
}

impl NextPhase for WalkPhase {
    fn next(self) -> Option<WalkPhase> {
        match self {
            WalkPhase::Empty => Some(WalkPhase::R0),
            WalkPhase::R0 => Some(WalkPhase::R1),
            WalkPhase::R1 => Some(WalkPhase::R3),
            WalkPhase::R2a => None,
            WalkPhase::R3 => Some(WalkPhase::R4a),
            WalkPhase::R4a => Some(WalkPhase::R4c),
            WalkPhase::R4b => Some(WalkPhase::R4c),
            WalkPhase::R4c => Some(WalkPhase::R5),
            WalkPhase::R5 => Some(WalkPhase::R6),
            WalkPhase::R6 => Some(WalkPhase::R7),
            WalkPhase::R7 => None,
            WalkPhase::R8 => Some(WalkPhase::R9),
            WalkPhase::R9 => Some(WalkPhase::R10),
            WalkPhase::R10 => None,
            WalkPhase::R11a => Some(WalkPhase::R12),
            WalkPhase::R11 => Some(WalkPhase::R12),
            WalkPhase::R12 => Some(WalkPhase::R13a),
            WalkPhase::R13a => Some(WalkPhase::R14a),
            WalkPhase::R13b => Some(WalkPhase::R14b),
            WalkPhase::R14a => None,
            WalkPhase::R14b => None,
        }
    }
}

fn ensure_supported_target(target: WalkPhase) -> Result<(), PrepareError> {
    if target.is_early_boundary() {
        Ok(())
    } else {
        Err(PrepareError::InvalidBatchSelection {
            detail: format!("walk target {target} is not supported by the current server slice"),
        })
    }
}

fn push_changes(lines: &mut Vec<String>, from: WalkPhase, to: WalkPhase, label: &str) {
    lines.push(format!("{label}:"));
    for change in to.changes_from(from) {
        let mut change_lines = change.lines();
        if let Some(first) = change_lines.next() {
            lines.push(format!("  - {first}"));
            for line in change_lines {
                lines.push(format!("    {line}"));
            }
        }
    }
}

fn push_colored_changes(
    lines: &mut Vec<String>,
    from: WalkPhase,
    to: WalkPhase,
    style: DeltaRenderStyle,
) {
    lines.push("typestate changes:".to_string());
    for delta in to.axis_deltas_from(from) {
        lines.push(format!(
            "  - {}",
            highlight_changed(delta.label, style.color)
        ));
        lines.push(format!(
            "    {} {}",
            highlight_removed("-", style.color),
            highlight_removed(&delta.from, style.color)
        ));
        lines.push(format!(
            "    {} {}",
            highlight_added("+", style.color),
            highlight_added(&delta.to, style.color)
        ));
    }
    push_side_effects(lines, from, to, style);
}

fn push_verbose_changes(
    lines: &mut Vec<String>,
    from: WalkPhase,
    to: WalkPhase,
    style: DeltaRenderStyle,
) {
    lines.push("typestate changes:".to_string());
    for delta in to.axis_deltas_from(from) {
        lines.push(format!(
            "  - {}",
            highlight_changed(delta.label, style.color)
        ));
        lines.push("    removed:".to_string());
        lines.push(format!(
            "      {}",
            highlight_removed(&delta.from, style.color)
        ));
        lines.push("    added:".to_string());
        lines.push(format!("      {}", highlight_added(&delta.to, style.color)));
        if delta.label != "phase" {
            let removed = delta.removed_structures();
            if !removed.is_empty() {
                lines.push("    structures removed:".to_string());
                for item in removed {
                    lines.push(format!(
                        "      {} {}",
                        highlight_removed("-", style.color),
                        highlight_removed(&item, style.color)
                    ));
                }
            }
            let added = delta.added_structures();
            if !added.is_empty() {
                lines.push("    structures added:".to_string());
                for item in added {
                    lines.push(format!(
                        "      {} {}",
                        highlight_added("+", style.color),
                        highlight_added(&item, style.color)
                    ));
                }
            }
        }
    }
    push_side_effects(lines, from, to, style);
}

fn push_side_effects(
    lines: &mut Vec<String>,
    from: WalkPhase,
    to: WalkPhase,
    style: DeltaRenderStyle,
) {
    if matches!((from, to), (WalkPhase::R4c, WalkPhase::R5)) {
        lines.push(format!(
            "  - {}: appends parent-start/resource entries to the transition journal",
            highlight_changed("side effect", style.color)
        ));
    }
    if matches!((from, to), (WalkPhase::R5, WalkPhase::R6)) {
        lines.push(format!(
            "  - {}: may advance eval/protocol closure before loading parent baseline",
            highlight_changed("side effect", style.color)
        ));
    }
    if matches!((from, to), (WalkPhase::R7, WalkPhase::R8)) {
        lines.push(format!(
            "  - {}: may publish or receive child-plan authority and wait on provider/harness work",
            highlight_changed("side effect", style.color)
        ));
    }
    if matches!((from, to), (WalkPhase::R10, WalkPhase::R11a)) {
        lines.push(format!(
            "  - {}: projects rejected-only selection evidence without child fanout",
            highlight_changed("side effect", style.color)
        ));
    }
    if matches!((from, to), (WalkPhase::R10, WalkPhase::R11)) {
        lines.push(format!(
            "  - {}: runs live child fanout and may spawn or observe child runtimes",
            highlight_changed("side effect", style.color)
        ));
    }
    if matches!(
        (from, to),
        (WalkPhase::R11a | WalkPhase::R11, WalkPhase::R12)
    ) {
        lines.push(format!(
            "  - {}: assembles report facts without emitting the final report",
            highlight_changed("projection", style.color)
        ));
    }
    if matches!((from, to), (WalkPhase::R12, WalkPhase::R13a)) {
        lines.push(format!(
            "  - {}: may record no-selection stopped continuation",
            highlight_changed("side effect", style.color)
        ));
    }
    if matches!((from, to), (WalkPhase::R12, WalkPhase::R13b)) {
        lines.push(format!(
            "  - {}: seals/appends History, installs selected successor checkout, retires parent, spawns successor, and waits for ready evidence",
            highlight_changed("side effect", style.color)
        ));
    }
    if matches!((from, to), (WalkPhase::R13a, WalkPhase::R14a)) {
        lines.push(format!(
            "  - {}: emits final report and records parent-complete evidence",
            highlight_changed("side effect", style.color)
        ));
    }
    if matches!((from, to), (WalkPhase::R13b, WalkPhase::R14b)) {
        lines.push(format!(
            "  - {}: emits final handoff report and records parent-complete evidence",
            highlight_changed("side effect", style.color)
        ));
    }
}

fn highlight_removed(value: &str, color: bool) -> String {
    paint(value, color, "31")
}

fn highlight_added(value: &str, color: bool) -> String {
    paint(value, color, "32")
}

fn highlight_changed(value: &str, color: bool) -> String {
    paint(value, color, "1;33")
}

fn paint(value: &str, color: bool, code: &str) -> String {
    if color {
        format!("\x1b[{code}m{value}\x1b[0m")
    } else {
        value.to_string()
    }
}

fn push_next_steps(lines: &mut Vec<String>, phase: WalkPhase) {
    lines.push("next:".to_string());
    let steps = phase.next_steps();
    if steps.is_empty() {
        lines.push("  (no admitted next step in this server slice)".to_string());
        return;
    }
    for step in steps {
        lines.push(format!(
            "  {} -> {} - {}",
            step.edge, step.phase, step.detail
        ));
    }
}

fn indent_lines(value: &str, spaces: usize) -> Vec<String> {
    let prefix = " ".repeat(spaces);
    value
        .lines()
        .map(|line| format!("{prefix}{line}"))
        .collect()
}

#[derive(Debug, Clone)]
struct LlmLane {
    lane_id: String,
    session: ToolLoopSession,
    resume: Option<ToolLoopResume>,
    head: Option<usize>,
}

fn lane_label(session: &ToolLoopSession) -> String {
    session
        .lane_id
        .clone()
        .or_else(|| {
            session
                .workspace
                .file_name()
                .and_then(|name| name.to_str())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| session.session_id.clone())
}

fn lane_sort_key(lane: &LlmLane) -> (usize, usize, String) {
    (
        lane.head.unwrap_or(0),
        lane.resume
            .as_ref()
            .map(|resume| resume.next_step)
            .unwrap_or(0),
        lane.session.session_id.clone(),
    )
}

fn status_label(status: ToolLoopStatus) -> &'static str {
    match status {
        ToolLoopStatus::Active => "active",
        ToolLoopStatus::Paused => "paused",
        ToolLoopStatus::Terminal => "terminal",
        ToolLoopStatus::Abandoned => "abandoned",
    }
}

fn outcome_label(outcome: &ToolLoopOutcome) -> String {
    match outcome {
        ToolLoopOutcome::ToolCalls {
            count,
            finish_reason,
            ..
        } => format!("tool_calls count={count} finish_reason={finish_reason}"),
        ToolLoopOutcome::Content { .. } => "content".to_string(),
    }
}

fn result_label(result: &ToolLoopResult) -> String {
    match result {
        ToolLoopResult::Completed(record) => format!(
            "completed tool={} call_id={} latency_ms={}",
            record.tool, record.call_id, record.latency_ms
        ),
        ToolLoopResult::Failed(record) => format!(
            "failed tool={} call_id={} latency_ms={} error={}",
            record.tool.as_deref().unwrap_or("-"),
            record.call_id,
            record.latency_ms,
            preview_inline(&record.error)
        ),
    }
}

fn workspace_label(state: &WorkspaceState) -> String {
    match (state.dirty_paths.is_empty(), state.error.as_deref()) {
        (true, None) => "clean".to_string(),
        (dirty_empty, error) => {
            let mut parts = Vec::new();
            if !dirty_empty {
                parts.push(format!("dirty_paths={}", state.dirty_paths.len()));
            }
            if let Some(error) = error {
                parts.push(format!("error={}", preview_inline(error)));
            }
            parts.join(" ")
        }
    }
}

fn preview_inline(value: &str) -> String {
    let mut preview = value.replace('\n', " ");
    if preview.len() > 120 {
        preview.truncate(117);
        preview.push_str("...");
    }
    preview
}

#[derive(Default)]
struct WalkPrevious {
    entries: Vec<String>,
}

impl WalkPrevious {
    fn push(&mut self, entry: String) {
        if self.entries.len() == MAX_HISTORY {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    fn lines(&self) -> Vec<String> {
        if self.entries.is_empty() {
            return vec!["  (empty)".to_string()];
        }
        self.entries
            .iter()
            .map(|entry| format!("  {entry}"))
            .collect()
    }
}

#[derive(Default)]
struct WalkFiles {
    root: Option<PathBuf>,
    tracking_dir: Option<PathBuf>,
    tracked: Vec<WalkFile>,
}

#[derive(Clone)]
struct WalkFile {
    label: &'static str,
    path: PathBuf,
}

impl WalkFiles {
    fn clear(&mut self) {
        self.root = None;
        self.tracking_dir = None;
        self.tracked.clear();
    }

    fn reset(&mut self, repo_root: &Path) {
        self.clear();
        self.root = Some(repo_root.to_path_buf());
        self.push("parent_identity", parent_identity_path(repo_root));
        if let Ok(path) = prototype1_monitor_target_file() {
            self.tracking_dir = path.parent().map(Path::to_path_buf);
            self.push("active_monitor_target", path);
        }
    }

    fn remember_campaign(&mut self, campaign_id: &CampaignId) {
        let Ok(manifest) = campaign_manifest_path_for_id(campaign_id) else {
            return;
        };
        self.push("campaign_manifest", manifest.clone());
        self.push(
            "transition_journal",
            prototype1_transition_journal_path(&manifest),
        );
    }

    fn push(&mut self, label: &'static str, path: PathBuf) {
        if self
            .tracked
            .iter()
            .any(|file| file.label == label && file.path == path)
        {
            return;
        }
        self.tracked.push(WalkFile { label, path });
    }

    fn push_roots(&self, lines: &mut Vec<String>) {
        lines.push(format!(
            "root: {}",
            self.root
                .as_deref()
                .map(display_dir)
                .unwrap_or_else(|| "-".to_string())
        ));
        lines.push(format!(
            "tracking_dir: {}",
            self.tracking_dir
                .as_deref()
                .map(display_dir)
                .unwrap_or_else(|| "-".to_string())
        ));
    }

    fn push_tracked(&self, lines: &mut Vec<String>) {
        if self.tracked.is_empty() {
            lines.push("tracked files: (none)".to_string());
            return;
        }
        lines.push("tracked files:".to_string());
        for file in &self.tracked {
            lines.push(format!(
                "  {}: {}",
                file.label,
                self.display_tracked(&file.path)
            ));
        }
    }

    fn render(&self) -> String {
        if self.tracked.is_empty() {
            return "no tracked output files yet; start a walk first".to_string();
        }
        let mut lines = vec!["tracked output files:".to_string()];
        for file in &self.tracked {
            lines.push(format!("--- {}: {} ---", file.label, file.path.display()));
            lines.push(preview_file(&file.path));
        }
        lines.join("\n")
    }

    fn display_tracked(&self, path: &Path) -> String {
        if let Some(root) = &self.root
            && let Ok(stripped) = path.strip_prefix(root)
        {
            return format!("{{root}}/{}", stripped.display());
        }
        if let Some(dir) = &self.tracking_dir
            && let Ok(stripped) = path.strip_prefix(dir)
        {
            return format!("{{tracking_dir}}/{}", stripped.display());
        }
        path.display().to_string()
    }
}

fn display_dir(path: &Path) -> String {
    let mut value = path.display().to_string();
    if !value.ends_with(std::path::MAIN_SEPARATOR) {
        value.push(std::path::MAIN_SEPARATOR);
    }
    value
}

fn preview_file(path: &Path) -> String {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return "(missing)".to_string();
        }
        Err(error) => return format!("(metadata error: {error})"),
    };
    if !metadata.is_file() {
        return format!("(not a regular file; {} bytes)", metadata.len());
    }
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) => return format!("(open error: {error})"),
    };
    let mut bytes = Vec::new();
    let read = file
        .by_ref()
        .take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes);
    if let Err(error) = read {
        return format!("(read error: {error})");
    }
    let truncated = bytes.len() > MAX_FILE_BYTES;
    if truncated {
        bytes.truncate(MAX_FILE_BYTES);
    }
    let mut header = format!("{} bytes", metadata.len());
    if truncated {
        header.push_str(&format!("; showing first {MAX_FILE_BYTES} bytes"));
    }
    if bytes.is_empty() {
        return format!("{header}\n(empty)");
    }
    format!("{header}\n{}", String::from_utf8_lossy(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    use ploke_llm::manager::{RecordedResponse, RequestMessage, ResponseIndex};
    use ploke_records::llm_response::RawFullResponseRecord;
    use uuid::Uuid;

    use crate::{
        cli::prototype1_state::identity::{ParentIdentity, write_parent_identity},
        replay::tool_loop::{
            FsToolLoopStore, ToolLoopResume, ToolLoopSession, ToolLoopStep, ToolLoopStore,
            WorkspaceState,
        },
        test_support::env_guard_os,
    };

    fn content_response(index: usize) -> RawFullResponseRecord {
        let response = serde_json::from_value(serde_json::json!({
            "id": format!("chatcmpl-walk-llm-{index}"),
            "choices": [{
                "index": 0,
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": "done"
                }
            }],
            "created": index,
            "model": "test/model",
            "object": "chat.completion"
        }))
        .expect("response json");
        RawFullResponseRecord {
            assistant_message_id: Uuid::from_u128(0xbbbbbbbb_bbbb_bbbb_bbbb_bbbbbbbbbbbb),
            recorded_response: RecordedResponse {
                response_index: ResponseIndex::new(index),
                response,
            },
        }
    }

    fn write_test_identity(repo: &Path, campaign: &CampaignId) {
        fs::create_dir_all(repo).expect("repo dir");
        let identity = ParentIdentity::root_bootstrap(
            campaign.clone(),
            "node-root",
            "instance-1",
            "branch-1",
            None,
        );
        write_parent_identity(repo, &identity).expect("parent identity");
    }

    #[test]
    fn llm_report_reads_latest_tool_loop_checkpoint() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let _guard = env_guard_os(vec![("PLOKE_EVAL_HOME", OsString::from(&eval_home))]);
        let campaign = CampaignId::from("campaign-walk-llm-test");
        let repo = tmp.path().join("repo");
        write_test_identity(&repo, &campaign);

        let store = FsToolLoopStore::new(
            eval_home
                .join("campaigns")
                .join(campaign.as_str())
                .join("prototype1/debug/tool-loop"),
        );
        let mut session = ToolLoopSession::new("session-1", "headless-tui", repo.clone());
        session.outer_phase = Some("r10".to_string());
        session.outer_edge = Some("r10->r11".to_string());
        session.status = ToolLoopStatus::Paused;
        store.write_session(&session).expect("write session");
        let step = ToolLoopStep::new(
            "session-1",
            0,
            vec![RequestMessage::new_user("hello".to_string())],
            content_response(0),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("step");
        store.write_step(&step).expect("write step");
        let mut resume = ToolLoopResume::new(
            "session-1",
            Uuid::from_u128(0xbbbbbbbb_bbbb_bbbb_bbbb_bbbbbbbbbbbb).to_string(),
            "parent-1",
            "request-1",
            vec![RequestMessage::new_user("hello".to_string())],
        );
        resume.next_step = 1;
        store.write_resume(&resume).expect("write resume");

        let report = WalkController::new(repo)
            .llm_report(None, None, false, None)
            .expect("llm report");

        assert!(report.contains("llm tool-loop checkpoint"));
        assert!(report.contains("session: session-1"));
        assert!(report.contains("status: paused"));
        assert!(report.contains("outer_edge: r10->r11"));
        assert!(report.contains("step: 0"));
        assert!(report.contains("response_index: 0"));
        assert!(report.contains("outcome: content"));
        assert!(report.contains("workspace_after: clean"));
    }

    #[test]
    fn llm_lane_cursor_moves_read_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let _guard = env_guard_os(vec![("PLOKE_EVAL_HOME", OsString::from(&eval_home))]);
        let campaign = CampaignId::from("campaign-walk-lanes-test");
        let repo = tmp.path().join("repo");
        write_test_identity(&repo, &campaign);

        let store = FsToolLoopStore::new(
            eval_home
                .join("campaigns")
                .join(campaign.as_str())
                .join("prototype1/debug/tool-loop"),
        );
        let mut session = ToolLoopSession::new("session-a", "headless-tui", repo.join("lane-a"));
        session.lane_id = Some("lane-a".to_string());
        session.status = ToolLoopStatus::Paused;
        store.write_session(&session).expect("write session");
        for index in 0..3 {
            let step = ToolLoopStep::new(
                "session-a",
                index,
                vec![RequestMessage::new_user("hello".to_string())],
                content_response(index),
                WorkspaceState::default(),
                WorkspaceState::default(),
            )
            .expect("step");
            store.write_step(&step).expect("write step");
        }
        let mut resume = ToolLoopResume::new(
            "session-a",
            Uuid::from_u128(0xbbbbbbbb_bbbb_bbbb_bbbb_bbbbbbbbbbbb).to_string(),
            "parent-1",
            "request-1",
            vec![RequestMessage::new_user("hello".to_string())],
        );
        resume.next_step = 3;
        store.write_resume(&resume).expect("write resume");

        let mut controller = WalkController::new(repo);
        let lanes = controller.llm_lanes_report(false).expect("lanes");
        assert!(lanes.contains("lane-a status=paused cursor=- head=2 next_step=3"));

        let focus = controller.llm_focus("lane-a".to_string()).expect("focus");
        assert!(focus.contains("focused llm lane lane-a"));

        let moved = controller
            .llm_move(Some("lane-a"), 1, LlmMove::Back)
            .expect("back");
        assert!(moved.contains("cursor=1 head=2"));
        let report = controller
            .llm_report(None, None, false, None)
            .expect("cursor report");
        assert!(report.contains("cursor: 1"));
        assert!(report.contains("step: 1"));

        controller
            .llm_move(Some("lane-a"), 1, LlmMove::Forward)
            .expect("forward");
        let report = controller
            .llm_report(None, Some("lane-a"), false, None)
            .expect("forward report");
        assert!(report.contains("cursor: 2"));
        assert!(report.contains("step: 2"));

        controller
            .llm_move(Some("lane-a"), 1, LlmMove::Back)
            .expect("back again");
        let head = controller.llm_head(Some("lane-a")).expect("head");
        assert!(head.contains("cursor=head (2)"));
        let report = controller
            .llm_report(None, Some("lane-a"), true, None)
            .expect("head report");
        assert!(report.contains("cursor: 2"));
        assert!(report.contains("step: 2"));
    }
}
