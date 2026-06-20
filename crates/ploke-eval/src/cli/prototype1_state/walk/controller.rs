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
    process::Command,
    str::FromStr,
};

use ploke_llm::{ModelId, ProviderKey, manager::Role};
use ploke_records::{
    agent_turn::{ToolCompletedRecord, ToolFailedRecord, ToolRequestRecord},
    ids::CampaignId,
    llm_response::RawFullResponseRecord,
    tool_contracts::{
        PersistedToolCallArguments, PersistedToolResultContent, ToolCallArguments, ToolErrorWire,
        ToolResultContent, ToolRetryContextValue, decode_tool_result_content,
    },
};
use ploke_tui::tools::{
    Tool, ToolDefinition, ToolName, cargo::CargoTool, code_edit::GatCodeEdit,
    code_item_lookup::CodeItemLookup, create_file::CreateFile, get_code_edges::CodeItemEdges,
    insert_rust_item::InsertRustItem, list_dir::ListDir, ns_patch::NsPatch, ns_read::NsRead,
    request_code_context::RequestCodeContextGat,
};

use crate::{
    ResolvedCampaignConfig,
    cli::prototype1_state::{
        cli_facing::{Prototype1StateRunShape, campaign_manifest_path_for_id},
        driver::{
            reconstruct::{self, EarlyState},
            replay::ReplayCursor,
        },
        edit_surface::{
            harness_request::PublishedBroadHarnessRequest,
            tui_adapter::{self, ModelSelection},
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
    cli::{
        Prototype1StateWalkLlmStepSource,
        provider::{headless_model_selection, load_parent_patcher_model_selection},
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
            reconstruction.push_lines(&mut lines, &self.files);
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
            let marker = if self.llm_focus.as_deref() == Some(lane.lane_id.as_str())
                || self.llm_focus.as_deref() == Some(lane.session.session_id.as_str())
            {
                "*"
            } else {
                " "
            };
            lines.push(format!(
                "{marker} {} status={} cursor={} head={} next_step={} model={}",
                lane.lane_id,
                status_label(lane.session.status),
                self.llm_cursors
                    .get(cursor_key(&lane))
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
        let focus_target = if lane == lane_state.session.session_id {
            lane_state.session.session_id.clone()
        } else {
            lane_state.lane_id.clone()
        };
        self.llm_focus = Some(focus_target);
        if let Some(head) = lane_state.head {
            self.llm_cursors
                .entry(cursor_key(&lane_state).to_string())
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
                .get(cursor_key(&lane_state))
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

    /// Render a compact chronological LLM/tool-loop checkpoint timeline.
    pub(crate) fn llm_timeline(
        &self,
        session_id: Option<&str>,
        lane: Option<&str>,
    ) -> Result<String, PrepareError> {
        let store = self.tool_loop_store()?;
        let lane_state = match session_id {
            Some(session_id) => self.lane_for_session(&store, store.read_session(session_id)?)?,
            None => self.resolve_lane(&store, lane)?,
        };
        self.render_llm_timeline(&store, lane_state)
    }

    /// Render the tool definition and historical arguments for one LLM tool call.
    pub(crate) fn llm_tool_report(
        &self,
        session_id: Option<&str>,
        lane: Option<&str>,
        head: bool,
        step: Option<usize>,
        call: Option<usize>,
        name: Option<&str>,
        json: bool,
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
                .get(cursor_key(&lane_state))
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
        self.render_llm_tool(lane_state, loaded, call, name, json)
    }

    /// Execute one historical or live LLM response step through current TUI tools.
    pub(crate) async fn llm_step(
        &mut self,
        session_id: Option<&str>,
        lane: Option<&str>,
        step: Option<usize>,
        source: Prototype1StateWalkLlmStepSource,
        watch: bool,
        allow_workspace_mutation: bool,
        model_id: Option<&str>,
        provider: Option<&str>,
        max_attempts: u32,
        timeout_secs: u64,
    ) -> Result<String, PrepareError> {
        let store = self.tool_loop_store()?;
        let lane_state = match session_id {
            Some(session_id) => self.lane_for_session(&store, store.read_session(session_id)?)?,
            None => self.resolve_lane(&store, lane)?,
        };
        let request = self.prepare_llm_step(
            &store,
            &lane_state,
            step,
            source,
            watch,
            allow_workspace_mutation,
            model_id,
            provider,
            max_attempts,
            timeout_secs,
        )?;
        let summary = self.run_llm_step_request(request).await?;
        let stepped_session = summary.session_id.to_string();
        self.focus_llm_session_head(&store, &stepped_session)?;
        Ok(summary.render())
    }

    fn focus_llm_session_head(
        &mut self,
        store: &FsToolLoopStore,
        session_id: &str,
    ) -> Result<(), PrepareError> {
        self.llm_focus = Some(session_id.to_string());
        if let Some(head) = store.latest_step_index(session_id)? {
            self.llm_cursors.insert(session_id.to_string(), head);
        }
        Ok(())
    }

    /// Continue live LLM response steps until terminal or max steps.
    pub(crate) async fn llm_finish(
        &mut self,
        session_id: Option<&str>,
        lane: Option<&str>,
        step: Option<usize>,
        watch: bool,
        allow_workspace_mutation: bool,
        model_id: Option<&str>,
        provider: Option<&str>,
        max_steps: usize,
        max_attempts: u32,
        timeout_secs: u64,
    ) -> Result<String, PrepareError> {
        if max_steps == 0 {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "walk llm finish --max-steps must be greater than zero".to_string(),
            });
        }
        let mut lines = vec!["llm live finish".to_string()];
        let mut current_session = session_id.map(ToOwned::to_owned);
        let mut current_lane = lane.map(ToOwned::to_owned);
        let mut current_step = step;
        for index in 0..max_steps {
            let store = self.tool_loop_store()?;
            let lane_state = match current_session.as_deref() {
                Some(session_id) => {
                    self.lane_for_session(&store, store.read_session(session_id)?)?
                }
                None => self.resolve_lane(&store, current_lane.as_deref())?,
            };
            let request = self.prepare_llm_step(
                &store,
                &lane_state,
                current_step,
                Prototype1StateWalkLlmStepSource::Live,
                watch,
                allow_workspace_mutation,
                model_id,
                provider,
                max_attempts,
                timeout_secs,
            )?;
            let summary = self.run_llm_step_request(request).await?;
            lines.push(format!("step {}:", index + 1));
            lines.extend(summary.render_indented("  "));
            let stepped_session = summary.session_id.to_string();
            self.focus_llm_session_head(&store, &stepped_session)?;
            current_session = Some(stepped_session);
            current_lane = Some(summary.lane_id.clone());
            current_step = None;
            if summary.terminal {
                lines.push("finish: terminal inner frame reached".to_string());
                return Ok(lines.join("\n"));
            }
        }
        lines.push(format!(
            "finish: stopped after max_steps={} before terminal inner frame",
            max_steps
        ));
        Ok(lines.join("\n"))
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
            .get(cursor_key(&lane_state))
            .copied()
            .unwrap_or(head);
        let next = match direction {
            LlmMove::Back => current.saturating_sub(steps),
            LlmMove::Forward => current.saturating_add(steps).min(head),
        };
        self.llm_cursors
            .insert(cursor_key(&lane_state).to_string(), next);
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
        self.llm_cursors
            .insert(cursor_key(&lane_state).to_string(), head);
        Ok(format!(
            "llm lane {} cursor=head ({head})",
            lane_state.lane_id
        ))
    }

    fn render_llm_timeline(
        &self,
        store: &FsToolLoopStore,
        lane: LlmLane,
    ) -> Result<String, PrepareError> {
        let indices = store.step_indices(&lane.session.session_id)?;
        let current = self
            .llm_cursors
            .get(cursor_key(&lane))
            .copied()
            .or(lane.head);
        let mut lines = Vec::new();
        lines.push("llm tool-loop timeline".to_string());
        lines.push(format!("store: {}", timeline_store_label(store.root())));
        lines.push(format!("lane: {}", lane.lane_id));
        lines.push(format!("session: {}", lane.session.session_id));
        lines.push(format!("status: {}", status_label(lane.session.status)));
        lines.extend(render_provenance_lines(
            &self.repo_root,
            &lane.session.workspace,
        ));
        lines.push(format!(
            "cursor: {}",
            current
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string())
        ));
        lines.push(format!(
            "head: {}",
            lane.head
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string())
        ));
        lines.push(format!("steps: {}", indices.len()));
        if indices.is_empty() {
            lines.push("timeline: (no recorded response steps)".to_string());
            return Ok(lines.join("\n"));
        }
        lines.push("timeline:".to_string());
        lines.push("  cur step  kind     action                              result".to_string());
        lines.push(
            "  --- ----- -------- ----------------------------------- ----------------------"
                .to_string(),
        );
        for index in indices {
            let step = store.read_step(&lane.session.session_id, index)?;
            let marker = if current == Some(index) { "*" } else { " " };
            lines.push(render_timeline_row(marker, &step));
        }
        lines.push("next:".to_string());
        lines.push("  walk llm show          # inspect the cursor row".to_string());
        lines.push("  walk llm back/forward  # move the cursor".to_string());
        lines.push("  walk llm head          # jump to latest recorded step".to_string());
        Ok(lines.join("\n"))
    }

    fn prepare_llm_step(
        &self,
        store: &FsToolLoopStore,
        lane: &LlmLane,
        step: Option<usize>,
        source: Prototype1StateWalkLlmStepSource,
        watch: bool,
        allow_workspace_mutation: bool,
        model_id: Option<&str>,
        provider: Option<&str>,
        max_attempts: u32,
        timeout_secs: u64,
    ) -> Result<LlmStepRequest, PrepareError> {
        if !allow_workspace_mutation {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "walk llm step executes current TUI tools and may mutate the candidate workspace; rerun with `--allow workspace-mutation`".to_string(),
            });
        }
        if source == Prototype1StateWalkLlmStepSource::Live && !watch {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "walk llm step --source live calls the provider; rerun with `--watch`"
                    .to_string(),
            });
        }
        if provider.is_some() && model_id.is_none() {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "walk llm --provider requires --model-id".to_string(),
            });
        }
        let budget = tui_adapter::Budget::new(max_attempts, timeout_secs).map_err(|source| {
            PrepareError::DatabaseSetup {
                phase: "walk_llm_budget",
                detail: source.to_string(),
            }
        })?;
        let published = self.load_llm_published_request(lane)?;
        let model = resolve_llm_model(lane.session.model.as_deref(), model_id, provider)?;
        let selected = self.select_llm_step_messages(store, lane, step, source)?;
        Ok(LlmStepRequest {
            lane_id: lane.lane_id.clone(),
            source,
            selected_step: selected.selected_step,
            workspace: lane.session.workspace.clone(),
            messages: selected.messages,
            recorded_response: selected.recorded_response,
            budget,
            surface: published.request().edit_policy.clone(),
            evidence: published.request().evidence_roots.clone(),
            model,
        })
    }

    async fn run_llm_step_request(
        &self,
        request: LlmStepRequest,
    ) -> Result<LlmStepSummary, PrepareError> {
        let source = match request.recorded_response {
            Some(response) => tui_adapter::LlmDebugStepSource::Recorded(response),
            None => tui_adapter::LlmDebugStepSource::Live,
        };
        let run = tui_adapter::run_llm_debug_step(
            &request.workspace,
            request.messages,
            request.budget,
            &request.surface,
            &request.evidence,
            Some(request.model),
            source,
        )
        .await
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "walk_llm_step",
            detail: source.to_string(),
        })?;
        let store = self.tool_loop_store()?;
        let session_id = run.session_id.to_string();
        let session = store.read_session(&session_id)?;
        let head = store.latest_step_index(&session_id)?;
        let terminal = session.status == ToolLoopStatus::Terminal
            || store
                .read_resume(&session_id)
                .map(|resume| resume.terminal)
                .unwrap_or(false);
        Ok(LlmStepSummary {
            lane_id: request.lane_id,
            source: request.source,
            selected_step: request.selected_step,
            session_id: run.session_id,
            outcome: run.outcome,
            attempts: run.attempts,
            final_messages: run.final_messages,
            head,
            terminal,
        })
    }

    fn select_llm_step_messages(
        &self,
        store: &FsToolLoopStore,
        lane: &LlmLane,
        step: Option<usize>,
        source: Prototype1StateWalkLlmStepSource,
    ) -> Result<LlmStepSelection, PrepareError> {
        let head = lane
            .head
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!("llm lane '{}' has no recorded steps", lane.lane_id),
            })?;
        let selected_step = step
            .or_else(|| self.llm_cursors.get(cursor_key(lane)).copied())
            .unwrap_or(head);
        match source {
            Prototype1StateWalkLlmStepSource::Historical => {
                let record = store.read_step(&lane.session.session_id, selected_step)?;
                Ok(LlmStepSelection {
                    selected_step,
                    messages: record.request_messages.clone(),
                    recorded_response: Some(record.response.clone()),
                })
            }
            Prototype1StateWalkLlmStepSource::Live => {
                if let Ok(next) =
                    store.read_step(&lane.session.session_id, selected_step.saturating_add(1))
                {
                    return Ok(LlmStepSelection {
                        selected_step,
                        messages: next.request_messages.clone(),
                        recorded_response: None,
                    });
                }
                let resume = store.read_resume(&lane.session.session_id)?;
                if resume.terminal {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "llm lane '{}' session {} is terminal at step {}; choose an earlier --step to branch live before terminal",
                            lane.lane_id, lane.session.session_id, selected_step
                        ),
                    });
                }
                Ok(LlmStepSelection {
                    selected_step,
                    messages: resume.request_messages,
                    recorded_response: None,
                })
            }
        }
    }

    fn load_llm_published_request(
        &self,
        lane: &LlmLane,
    ) -> Result<PublishedBroadHarnessRequest, PrepareError> {
        let path = match lane.session.request_path.clone() {
            Some(path) => path,
            None => self.infer_llm_request_path(&lane.lane_id)?,
        };
        let json = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
            path: path.clone(),
            source,
        })?;
        serde_json::from_str(&json).map_err(|source| PrepareError::DatabaseSetup {
            phase: "walk_llm_request",
            detail: format!(
                "failed to parse published request '{}': {source}",
                path.display()
            ),
        })
    }

    fn infer_llm_request_path(&self, lane_id: &str) -> Result<PathBuf, PrepareError> {
        let identity = load_parent_identity_optional(&self.repo_root)?.ok_or_else(|| {
            PrepareError::DatabaseSetup {
                phase: "walk_llm_request",
                detail: format!(
                    "parent identity is required to infer edit-harness request path: {}",
                    parent_identity_path(&self.repo_root).display()
                ),
            }
        })?;
        let manifest = campaign_manifest_path_for_id(identity.campaign_id())?;
        let campaign_dir = manifest
            .parent()
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "walk_llm_request",
                detail: format!(
                    "campaign manifest '{}' has no parent directory",
                    manifest.display()
                ),
            })?;
        Ok(campaign_dir
            .join("prototype1/messages/edit-harness-request")
            .join(format!("{lane_id}.json")))
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
            if let Ok(session) = store.read_session(target) {
                return self.lane_for_session(store, session);
            }
            return lanes
                .into_iter()
                .find(|lane| lane.lane_id == target || lane.session.session_id == target)
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "unknown llm lane or session '{target}'; run `walk llm lanes` to list available lanes"
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
        lines.extend(render_provenance_lines(
            &self.repo_root,
            &lane.session.workspace,
        ));
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
                .get(cursor_key(&lane))
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
            Some(ref resume) => {
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
                lines.extend(render_step_transcript(&record));
                lines.push(format!("terminal: {}", record.terminal));
                lines.push(format!(
                    "workspace_before: {}",
                    workspace_label(&record.workspace_before)
                ));
                lines.push(format!(
                    "workspace_after: {}",
                    workspace_label(&record.workspace_after)
                ));
                lines.extend(render_llm_next_commands(&lane, &record));
            }
            None => lines.push("step: (none recorded yet)".to_string()),
        }
        lines.join("\n")
    }

    fn render_llm_tool(
        &self,
        lane: LlmLane,
        loaded: Option<(usize, crate::replay::tool_loop::ToolLoopStep)>,
        call: Option<usize>,
        name: Option<&str>,
        json: bool,
    ) -> Result<String, PrepareError> {
        let selected_call = match loaded.as_ref() {
            Some((_, record)) => select_tool_request(record, call, name)?,
            None => None,
        };
        let tool_name = selected_call
            .map(|request| request.tool.as_str())
            .or(name)
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail:
                    "selected LLM step has no tool calls; pass --name to inspect a tool definition"
                        .to_string(),
            })?;
        let definition = current_tool_definition(tool_name);
        if json {
            return render_llm_tool_json(
                self,
                &lane,
                loaded.as_ref().map(|(step, _)| *step),
                selected_call,
                tool_name,
                definition.as_ref(),
            );
        }

        let mut lines = Vec::new();
        lines.push("llm tool".to_string());
        lines.push(format!("lane: {}", lane.lane_id));
        lines.push(format!("session: {}", lane.session.session_id));
        if let Some((step, _)) = loaded.as_ref() {
            lines.push(format!("step: {step}"));
        } else {
            lines.push("step: (none selected)".to_string());
        }
        lines.push(format!("tool: {tool_name}"));
        lines.extend(render_provenance_lines(
            &self.repo_root,
            &lane.session.workspace,
        ));
        lines.push("definition_source: current_renderer_checkout".to_string());
        if definition.is_none() {
            lines.push("definition: (unknown tool in current renderer)".to_string());
        }
        if loaded.is_some() {
            lines.push("arguments_source: persisted_checkpoint".to_string());
        }
        if let Some(request) = selected_call {
            lines.push(format!("call_id: {}", request.call_id));
            lines.push("arguments:".to_string());
            lines.extend(indent_lines(&format_tool_arguments(request), 2));
            lines.push("arguments_json:".to_string());
            lines.extend(indent_lines(
                &pretty_json_or_raw(request.arguments.as_str()),
                2,
            ));
        } else if name.is_some() {
            lines.push("arguments: (no matching persisted call at selected step)".to_string());
        }
        if let Some(definition) = definition.as_ref() {
            lines.extend(render_tool_definition(definition));
        }
        lines.push("next:".to_string());
        lines.push("  walk llm show      # inspect the full checkpoint transcript".to_string());
        lines.push("  walk llm tool --json".to_string());
        Ok(lines.join("\n"))
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
    fn push_lines(&self, lines: &mut Vec<String>, files: &WalkFiles) {
        lines.push("reconstruction:".to_string());
        if self.notes.is_empty() && self.blockers.is_empty() {
            lines.push("  (no durable reconstruction notes)".to_string());
            return;
        }
        for note in &self.notes {
            let note = render_reconstruction_note(note, files);
            push_wrapped_item(lines, &note, "  - ", "    ");
        }
        for blocker in &self.blockers {
            let blocker = strip_reconstructed(blocker);
            push_wrapped_item(lines, &blocker, "  - blocked: ", "    ");
        }
    }
}

fn render_reconstruction_note(note: &str, files: &WalkFiles) -> String {
    if let Some(rest) = note.strip_prefix("parent identity: ") {
        return render_parent_note(rest, files);
    }
    if let Some(path) = note
        .strip_prefix("no parent identity found at '")
        .and_then(|value| value.strip_suffix('\''))
    {
        return format!(
            "parent_identity: {} (missing)",
            files.display_tracked(Path::new(path))
        );
    }
    if let Some(path) =
        note.strip_prefix("inferred successor handoff invocation from durable journal: ")
    {
        return format!(
            "successor_handoff_invocation: {}",
            files.display_tracked(Path::new(path))
        );
    }
    strip_reconstructed(note)
}

fn render_parent_note(note: &str, files: &WalkFiles) -> String {
    let Some((path, rest)) = note.split_once(" (") else {
        return format!(
            "parent_identity: {}",
            files.display_tracked(Path::new(note))
        );
    };
    let detail = rest.strip_suffix(')').unwrap_or(rest);
    format!(
        "parent_identity: {}\n{detail}",
        files.display_tracked(Path::new(path))
    )
}

fn strip_reconstructed(note: &str) -> String {
    let Some(rest) = note.strip_prefix("reconstructed ") else {
        return note.to_string();
    };
    for separator in [" from ", " by "] {
        if let Some((phase, detail)) = rest.split_once(separator) {
            return format!("{phase}: {detail}");
        }
    }
    rest.to_string()
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
    let start = lines.len();
    let style = DeltaRenderStyle {
        verbose: false,
        color: false,
    };
    for delta in to.axis_deltas_from(from) {
        push_axis_delta(lines, &delta, style, 2);
    }
    push_side_effects(lines, from, to, style);
    if lines.len() == start {
        lines.push("  - no admitted typestate delta for this phase pair".to_string());
    }
}

fn push_colored_changes(
    lines: &mut Vec<String>,
    from: WalkPhase,
    to: WalkPhase,
    style: DeltaRenderStyle,
) {
    lines.push("typestate changes:".to_string());
    let start = lines.len();
    for delta in to.axis_deltas_from(from) {
        push_axis_delta(lines, &delta, style, 2);
    }
    push_side_effects(lines, from, to, style);
    if lines.len() == start {
        lines.push("  - no admitted typestate delta for this phase pair".to_string());
    }
}

fn push_verbose_changes(
    lines: &mut Vec<String>,
    from: WalkPhase,
    to: WalkPhase,
    style: DeltaRenderStyle,
) {
    lines.push("typestate changes:".to_string());
    let start = lines.len();
    for delta in to.axis_deltas_from(from) {
        lines.push(format!(
            "  - {}:",
            highlight_changed(delta.label, style.color)
        ));
        lines.push("    removed:".to_string());
        push_type_lines(lines, &delta.from, 6, style.color, highlight_removed);
        lines.push("    added:".to_string());
        push_type_lines(lines, &delta.to, 6, style.color, highlight_added);
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
    if lines.len() == start {
        lines.push("  - no admitted typestate delta for this phase pair".to_string());
    }
}

fn push_axis_delta(
    lines: &mut Vec<String>,
    delta: &typestate::RuntimeAxisDelta,
    style: DeltaRenderStyle,
    indent: usize,
) {
    let compact = format!("{}: {} -> {}", delta.label, delta.from, delta.to);
    if compact.len() <= 96 {
        lines.push(format!(
            "{}- {}: {} -> {}",
            " ".repeat(indent),
            highlight_changed(delta.label, style.color),
            highlight_removed(&delta.from, style.color),
            highlight_added(&delta.to, style.color)
        ));
        return;
    }
    lines.push(format!(
        "{}- {}:",
        " ".repeat(indent),
        highlight_changed(delta.label, style.color)
    ));
    push_type_lines(
        lines,
        &delta.from,
        indent + 4,
        style.color,
        highlight_removed,
    );
    push_arrow_type(lines, &delta.to, indent + 4, style.color, highlight_added);
}

fn push_type_lines(
    lines: &mut Vec<String>,
    value: &str,
    indent: usize,
    color: bool,
    painter: fn(&str, bool) -> String,
) {
    for line in typestate::render_type_expr(value, indent) {
        lines.push(painter(&line, color));
    }
}

fn push_arrow_type(
    lines: &mut Vec<String>,
    value: &str,
    indent: usize,
    color: bool,
    painter: fn(&str, bool) -> String,
) {
    let mut rendered = typestate::render_type_expr(value, indent).into_iter();
    if let Some(first) = rendered.next() {
        lines.push(painter(
            &format!("{}-> {}", " ".repeat(indent), first.trim_start()),
            color,
        ));
    }
    for line in rendered {
        lines.push(painter(&line, color));
    }
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

struct LlmStepRequest {
    lane_id: String,
    source: Prototype1StateWalkLlmStepSource,
    selected_step: usize,
    workspace: PathBuf,
    messages: Vec<ploke_tui::llm::RequestMessage>,
    recorded_response: Option<RawFullResponseRecord>,
    budget: tui_adapter::Budget,
    surface: crate::cli::prototype1_state::edit_surface::surface_policy::SurfacePolicy,
    evidence: Vec<crate::cli::prototype1_state::edit_surface::harness_request::EvidenceRoot>,
    model: ModelSelection,
}

struct LlmStepSelection {
    selected_step: usize,
    messages: Vec<ploke_tui::llm::RequestMessage>,
    recorded_response: Option<RawFullResponseRecord>,
}

struct LlmStepSummary {
    lane_id: String,
    source: Prototype1StateWalkLlmStepSource,
    selected_step: usize,
    session_id: uuid::Uuid,
    outcome: String,
    attempts: u32,
    final_messages: usize,
    head: Option<usize>,
    terminal: bool,
}

impl LlmStepSummary {
    fn render(&self) -> String {
        self.render_indented("").join("\n")
    }

    fn render_indented(&self, prefix: &str) -> Vec<String> {
        vec![
            format!("{prefix}llm step"),
            format!("{prefix}lane: {}", self.lane_id),
            format!("{prefix}source: {:?}", self.source),
            format!("{prefix}selected_step: {}", self.selected_step),
            format!("{prefix}new_session: {}", self.session_id),
            format!(
                "{prefix}new_head: {}",
                self.head
                    .map(|head| head.to_string())
                    .unwrap_or_else(|| "-".to_string())
            ),
            format!("{prefix}outcome: {}", self.outcome),
            format!("{prefix}attempts: {}", self.attempts),
            format!("{prefix}final_messages: {}", self.final_messages),
            format!("{prefix}terminal: {}", self.terminal),
            format!("{prefix}inspect: walk llm show"),
            format!(
                "{prefix}inspect_explicit: walk llm show --session-id {} --head",
                self.session_id
            ),
        ]
    }
}

#[derive(Debug, Clone)]
struct LlmLane {
    lane_id: String,
    session: ToolLoopSession,
    resume: Option<ToolLoopResume>,
    head: Option<usize>,
}

fn cursor_key(lane: &LlmLane) -> &str {
    lane.session.session_id.as_str()
}

fn resolve_llm_model(
    session_model: Option<&str>,
    requested_model: Option<&str>,
    requested_provider: Option<&str>,
) -> Result<ModelSelection, PrepareError> {
    let model = requested_model.or(session_model);
    match (model, requested_provider) {
        (Some(model), provider) => {
            let model_id = ModelId::from_str(model).map_err(|err| PrepareError::DatabaseSetup {
                phase: "walk_llm_model",
                detail: format!("invalid model id '{model}': {err}"),
            })?;
            let provider = provider
                .map(|provider| {
                    ProviderKey::new(provider).map_err(|err| PrepareError::DatabaseSetup {
                        phase: "walk_llm_provider",
                        detail: format!("invalid provider slug '{provider}': {err}"),
                    })
                })
                .transpose()?;
            headless_model_selection(model_id, provider)
        }
        (None, Some(provider)) => Err(PrepareError::InvalidBatchSelection {
            detail: format!("walk llm provider '{provider}' requires --model-id"),
        }),
        (None, None) => load_parent_patcher_model_selection(),
    }
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

fn render_timeline_row(marker: &str, step: &crate::replay::tool_loop::ToolLoopStep) -> String {
    let kind = timeline_kind(step);
    let action = timeline_action(step);
    let result = timeline_result(step);
    format!(
        "  {marker} #{:04} {:<8} {:<35} {}",
        step.step_index,
        kind,
        fit_cell(&action, 35),
        fit_cell(&result, 22)
    )
}

fn timeline_store_label(path: &Path) -> String {
    if path.ends_with("prototype1/debug/tool-loop") {
        "{campaign}/prototype1/debug/tool-loop".to_string()
    } else {
        path.display().to_string()
    }
}

fn timeline_kind(step: &crate::replay::tool_loop::ToolLoopStep) -> &'static str {
    if !step.tool_requests.is_empty() {
        "tool"
    } else if step.terminal {
        "content"
    } else {
        match &step.outcome {
            ToolLoopOutcome::Content { .. } => "content",
            ToolLoopOutcome::ToolCalls { .. } => "tool",
        }
    }
}

fn timeline_action(step: &crate::replay::tool_loop::ToolLoopStep) -> String {
    match step.tool_requests.as_slice() {
        [] => assistant_timeline_preview(step).unwrap_or_else(|| "assistant content".to_string()),
        [request] => tool_action_summary(request),
        requests => {
            let names = requests
                .iter()
                .map(|request| request.tool.as_str())
                .collect::<Vec<_>>()
                .join(",");
            format!("{} tool calls [{names}]", requests.len())
        }
    }
}

fn tool_action_summary(request: &ToolRequestRecord) -> String {
    match request.arguments.decode_for_tool(&request.tool) {
        PersistedToolCallArguments::Decoded(arguments) => match arguments {
            ToolCallArguments::RequestCodeContext(args) => format!(
                "{} {}",
                request.tool,
                args.search_term
                    .unwrap_or_else(|| "<no search>".to_string())
            ),
            ToolCallArguments::NsRead(args) => format!("{} {}", request.tool, args.file),
            ToolCallArguments::NsPatch(args) => {
                let files = args
                    .patches
                    .iter()
                    .take(3)
                    .map(|patch| patch.file.as_str())
                    .collect::<Vec<_>>()
                    .join(",");
                if args.patches.len() > 3 {
                    format!("{} {} (+{})", request.tool, files, args.patches.len() - 3)
                } else {
                    format!("{} {files}", request.tool)
                }
            }
            ToolCallArguments::Cargo(args) => format!("{} {:?}", request.tool, args.command),
            ToolCallArguments::ListDir(args) => {
                let dir = if args.dir.is_empty() { "." } else { &args.dir };
                format!("{} {dir}", request.tool)
            }
            _ => request.tool.clone(),
        },
        PersistedToolCallArguments::ParseFailure(failure) => {
            raw_tool_target(&request.tool, &failure.raw_arguments)
                .map(|target| format!("{} {target}", request.tool))
                .or_else(|| {
                    raw_json_fields(&failure.raw_arguments).map(|fields| {
                        format!("{} {}", request.tool, preview_inline_chars(&fields, 80))
                    })
                })
                .unwrap_or_else(|| request.tool.clone())
        }
    }
}

fn raw_tool_target(tool: &str, raw: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let object = value.as_object()?;
    match tool {
        "list_dir" => object
            .get("dir")
            .and_then(serde_json::Value::as_str)
            .map(|dir| if dir.is_empty() { "." } else { dir }.to_string()),
        "read_file" | "ns_read" => object
            .get("file")
            .or_else(|| object.get("file_path"))
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        "request_code_context" => object
            .get("search_term")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        "non_semantic_patch" | "ns_patch" => object
            .get("patches")
            .and_then(serde_json::Value::as_array)
            .map(|patches| {
                let files = patches
                    .iter()
                    .filter_map(|patch| patch.get("file").and_then(serde_json::Value::as_str))
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(",");
                if patches.len() > 3 {
                    format!("{} (+{})", files, patches.len() - 3)
                } else {
                    files
                }
            })
            .filter(|files| !files.is_empty()),
        "cargo" => object
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        _ => None,
    }
}

fn timeline_result(step: &crate::replay::tool_loop::ToolLoopStep) -> String {
    match step.tool_results.as_slice() {
        [] if step.terminal => "terminal".to_string(),
        [] => "no tool results".to_string(),
        [ToolLoopResult::Completed(record)] => completed_timeline_result(record),
        [ToolLoopResult::Failed(record)] => failed_timeline_result(record),
        results => {
            let failed = results
                .iter()
                .filter(|result| matches!(result, ToolLoopResult::Failed(_)))
                .count();
            if failed == 0 {
                format!("{} completed", results.len())
            } else {
                format!("{} results; {failed} failed", results.len())
            }
        }
    }
}

fn completed_timeline_result(record: &ToolCompletedRecord) -> String {
    match decode_tool_result_content(&record.tool, &record.content) {
        PersistedToolResultContent::Decoded(result) => match result {
            ToolResultContent::ListDir(result) => {
                format!("ok entries={}", result.entries.len())
            }
            ToolResultContent::Cargo(result) => format!(
                "ok {:?} e{} w{}",
                result.status_reason, result.summary.errors, result.summary.warnings
            ),
            ToolResultContent::NsPatch(result) => format!(
                "ok staged={} applied={} files={}",
                result.staged,
                result.applied,
                result.files.len()
            ),
            ToolResultContent::NsRead(result) => format!(
                "ok bytes={}{}",
                result
                    .byte_len
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                if result.truncated { " truncated" } else { "" }
            ),
            _ => "completed".to_string(),
        },
        PersistedToolResultContent::ParseFailure(_) => "completed raw".to_string(),
    }
}

fn failed_timeline_result(record: &ToolFailedRecord) -> String {
    if let Some(wire) = ToolErrorWire::parse(&record.error) {
        let protected = wire.user.contains("protected")
            || wire.llm.message.contains("protected")
            || wire.llm.received.as_deref() == Some("Cargo.toml");
        if protected {
            "failed protected-write".to_string()
        } else {
            format!("failed {:?}", wire.llm.code)
        }
    } else {
        "failed".to_string()
    }
}

fn assistant_timeline_preview(step: &crate::replay::tool_loop::ToolLoopStep) -> Option<String> {
    let message = first_response_message(step)?;
    let content = message.get("content")?.as_str()?.trim();
    if content.is_empty() || content == "Calling tools..." {
        None
    } else {
        Some(content.to_string())
    }
}

fn fit_cell(value: &str, width: usize) -> String {
    let single_line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.chars().count() <= width {
        return single_line;
    }
    if width <= 3 {
        single_line.chars().take(width).collect()
    } else {
        let prefix = single_line.chars().take(width - 3).collect::<String>();
        format!("{prefix}...")
    }
}

fn render_step_transcript(record: &crate::replay::tool_loop::ToolLoopStep) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("transcript:".to_string());
    lines.extend(indent_lines(
        &render_request_summary(&record.request_messages),
        2,
    ));
    lines.extend(indent_lines(&render_assistant_response(record), 2));
    lines.extend(indent_lines(&render_tool_calls(&record.tool_requests), 2));
    lines.extend(indent_lines(&render_tool_results(&record.tool_results), 2));
    lines
}

fn render_request_summary(messages: &[ploke_tui::llm::RequestMessage]) -> String {
    let mut system = 0usize;
    let mut user = 0usize;
    let mut assistant = 0usize;
    let mut tool = 0usize;
    for message in messages {
        match message.role {
            Role::System => system += 1,
            Role::User => user += 1,
            Role::Assistant => assistant += 1,
            Role::Tool => tool += 1,
        }
    }

    let mut lines = vec![format!(
        "request_messages: {} (system={system} user={user} assistant={assistant} tool={tool})",
        messages.len()
    )];
    if messages.is_empty() {
        return lines.join("\n");
    }
    lines.push("prior_messages:".to_string());
    let start = messages.len().saturating_sub(4);
    for (index, message) in messages.iter().enumerate().skip(start) {
        lines.push(format!("  #{index}: {}", request_message_summary(message)));
    }
    lines.join("\n")
}

fn request_message_summary(message: &ploke_tui::llm::RequestMessage) -> String {
    let mut parts = Vec::new();
    parts.push(format!("{}", role_label(message.role)));
    if let Some(call_id) = message.tool_call_id.as_ref() {
        parts.push(format!("tool_call_id={call_id}"));
    }
    if let Some(calls) = message.tool_calls.as_ref() {
        let names = calls
            .iter()
            .map(|call| call.function.name.as_str().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(format!("tool_calls=[{names}]"));
    }
    let content = preview_inline_chars(&message.content, 180);
    if !content.is_empty() {
        parts.push(content);
    }
    parts.join(" ")
}

fn role_label(role: Role) -> &'static str {
    match role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    }
}

fn render_assistant_response(record: &crate::replay::tool_loop::ToolLoopStep) -> String {
    let mut lines = vec!["assistant_response:".to_string()];
    if let Some(message) = first_response_message(record) {
        let content = message
            .get("content")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let reasoning = message
            .get("reasoning")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if !content.trim().is_empty() {
            lines.extend(indent_lines(&text_block("content", content, 1_600), 2));
        }
        if !reasoning.trim().is_empty() {
            lines.extend(indent_lines(&text_block("reasoning", reasoning, 1_000), 2));
        }
        if content.trim().is_empty() && reasoning.trim().is_empty() {
            lines.push("  (no assistant prose; response requested tools)".to_string());
        }
    } else {
        lines.push("  (provider message unavailable)".to_string());
    }
    lines.join("\n")
}

fn first_response_message(
    record: &crate::replay::tool_loop::ToolLoopStep,
) -> Option<serde_json::Value> {
    let value = serde_json::to_value(record.response.response()).ok()?;
    value
        .get("choices")?
        .as_array()?
        .first()?
        .get("message")
        .cloned()
}

fn select_tool_request<'a>(
    record: &'a crate::replay::tool_loop::ToolLoopStep,
    call: Option<usize>,
    name: Option<&str>,
) -> Result<Option<&'a ToolRequestRecord>, PrepareError> {
    if record.tool_requests.is_empty() {
        return Ok(None);
    }
    if let Some(call) = call {
        if call == 0 {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "--call is one-based; use --call 1 for the first tool call".to_string(),
            });
        }
        let index = call - 1;
        return record
            .tool_requests
            .get(index)
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "selected step has {} tool call(s); --call {call} is out of range",
                    record.tool_requests.len()
                ),
            })
            .map(Some);
    }
    if let Some(name) = name {
        if let Some(request) = record
            .tool_requests
            .iter()
            .find(|request| request.tool == name)
        {
            return Ok(Some(request));
        }
        return Ok(None);
    }
    Ok(record.tool_requests.first())
}

fn current_tool_definition(tool: &str) -> Option<ToolDefinition> {
    let name = parse_tool_name(tool)?;
    Some(match name {
        ToolName::RequestCodeContext => RequestCodeContextGat::tool_def(),
        ToolName::ApplyCodeEdit => GatCodeEdit::tool_def(),
        ToolName::InsertRustItem => InsertRustItem::tool_def(),
        ToolName::CreateFile => CreateFile::tool_def(),
        ToolName::NsPatch => NsPatch::tool_def(),
        ToolName::NsRead => NsRead::tool_def(),
        ToolName::CodeItemLookup => CodeItemLookup::tool_def(),
        ToolName::CodeItemEdges => CodeItemEdges::tool_def(),
        ToolName::Cargo => CargoTool::tool_def(),
        ToolName::ListDir => ListDir::tool_def(),
    })
}

fn parse_tool_name(tool: &str) -> Option<ToolName> {
    serde_json::from_value(serde_json::Value::String(tool.to_string())).ok()
}

fn render_provenance_lines(repo_root: &Path, workspace: &Path) -> Vec<String> {
    let Some(server_commit) = server_cwd_git_head() else {
        return Vec::new();
    };
    let Some(workspace_commit) = git_head(workspace) else {
        return Vec::new();
    };
    if server_commit == workspace_commit {
        return vec![format!(
            "provenance: server_cwd_commit={}",
            short_sha(&server_commit)
        )];
    }
    let target_commit = git_head(repo_root)
        .map(|commit| short_sha(&commit))
        .unwrap_or_else(|| "-".to_string());
    vec![
        format!(
            "provenance: server_cwd_commit={} checkpoint_workspace_commit={} target_repo_commit={}",
            short_sha(&server_commit),
            short_sha(&workspace_commit),
            target_commit
        ),
        "warning: rendered by a different checkout than the checkpoint workspace; decoded fields/tool hints/schemas may reflect current code, not the original run"
            .to_string(),
    ]
}

fn provenance_value(repo_root: &Path, workspace: &Path) -> serde_json::Value {
    let server_commit = server_cwd_git_head();
    let workspace_commit = git_head(workspace);
    let target_commit = git_head(repo_root);
    let commit_mismatch =
        server_commit.is_some() && workspace_commit.is_some() && server_commit != workspace_commit;
    serde_json::json!({
        "server_cwd_commit": server_commit,
        "checkpoint_workspace_commit": workspace_commit,
        "target_repo_commit": target_commit,
        "commit_mismatch": commit_mismatch,
        "warning": "decoded fields/tool hints/schemas are rendered by the current binary; when commits differ they may not match the original run",
    })
}

fn server_cwd_git_head() -> Option<String> {
    std::env::current_dir().ok().and_then(|dir| git_head(&dir))
}

fn git_head(path: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let commit = stdout.trim();
    if commit.is_empty() {
        None
    } else {
        Some(commit.to_string())
    }
}

fn short_sha(commit: &str) -> String {
    commit.chars().take(7).collect()
}

fn render_llm_tool_json(
    controller: &WalkController,
    lane: &LlmLane,
    step: Option<usize>,
    selected_call: Option<&ToolRequestRecord>,
    tool_name: &str,
    definition: Option<&ToolDefinition>,
) -> Result<String, PrepareError> {
    let arguments = selected_call.and_then(|request| {
        serde_json::from_str::<serde_json::Value>(request.arguments.as_str()).ok()
    });
    let payload = serde_json::json!({
        "kind": "llm_tool",
        "lane": lane.lane_id,
        "session": lane.session.session_id,
        "step": step,
        "tool": tool_name,
        "provenance": provenance_value(&controller.repo_root, &lane.session.workspace),
        "definition_source": "current_renderer_checkout",
        "tool_definition": definition,
        "call_id": selected_call.map(|request| request.call_id.as_str()),
        "arguments_source": selected_call.map(|_| "persisted_checkpoint"),
        "arguments": arguments,
        "arguments_raw": selected_call.map(|request| request.arguments.as_str()),
    });
    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)
}

fn render_tool_definition(definition: &ToolDefinition) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("description:".to_string());
    let description = truncate_chars(definition.function.description.trim(), 1_400);
    if description.is_empty() {
        lines.push("  (empty)".to_string());
    } else {
        lines.extend(description.lines().map(|line| format!("  {line}")));
    }
    lines.push("parameters:".to_string());
    render_schema_object(&mut lines, &definition.function.parameters, 2);
    lines
}

fn render_schema_object(lines: &mut Vec<String>, schema: &serde_json::Value, indent: usize) {
    let required = schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let Some(properties) = schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
    else {
        lines.push(format!("{}{}", " ".repeat(indent), schema_kind(schema)));
        return;
    };
    for (name, property) in properties {
        let mark = if required.iter().any(|item| item == name) {
            "required"
        } else {
            "optional"
        };
        lines.push(format!(
            "{}{}: {} {}",
            " ".repeat(indent),
            name,
            schema_kind(property),
            mark
        ));
        if let Some(description) = property
            .get("description")
            .and_then(serde_json::Value::as_str)
        {
            lines.push(format!(
                "{}- {}",
                " ".repeat(indent + 2),
                preview_inline_chars(description, 220)
            ));
        }
        if let Some(items) = property.get("items") {
            lines.push(format!("{}items:", " ".repeat(indent + 2)));
            render_schema_object(lines, items, indent + 4);
        } else if property.get("properties").is_some() {
            render_schema_object(lines, property, indent + 2);
        }
    }
}

fn schema_kind(schema: &serde_json::Value) -> String {
    match schema.get("type").and_then(serde_json::Value::as_str) {
        Some(kind) => kind.to_string(),
        None => "value".to_string(),
    }
}

fn pretty_json_or_raw(raw: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value) => serde_json::to_string_pretty(&value).unwrap_or_else(|_| raw.to_string()),
        Err(_) => raw.to_string(),
    }
}

fn render_tool_calls(requests: &[ToolRequestRecord]) -> String {
    let mut lines = vec![format!("tool_calls: {}", requests.len())];
    if requests.is_empty() {
        lines.push("  (none)".to_string());
        return lines.join("\n");
    }
    for (index, request) in requests.iter().enumerate() {
        lines.push(format!(
            "  {}. {} call_id={}",
            index + 1,
            request.tool,
            request.call_id
        ));
        lines.extend(indent_lines(&format_tool_arguments(request), 5));
    }
    lines.join("\n")
}

fn format_tool_arguments(request: &ToolRequestRecord) -> String {
    match request.arguments.decode_for_tool(&request.tool) {
        PersistedToolCallArguments::Decoded(arguments) => match arguments {
            ToolCallArguments::RequestCodeContext(args) => compact_fields(vec![
                field_opt("search_term", args.search_term),
                field_opt_u32("token_budget_per_result", args.token_budget_per_result),
                field_opt_u32("token_budget_total", args.token_budget_total),
            ]),
            ToolCallArguments::NsRead(args) => compact_fields(vec![
                Some(format!("file: {}", args.file)),
                field_opt_u32("start_line", args.start_line),
                field_opt_u32("end_line", args.end_line),
                field_opt_u32("max_bytes", args.max_bytes),
            ]),
            ToolCallArguments::NsPatch(args) => {
                let mut lines = Vec::new();
                if let Some(confidence) = args.confidence {
                    lines.push(format!("confidence: {confidence:.2}"));
                }
                lines.push(format!("patches: {}", args.patches.len()));
                for patch in args.patches.iter().take(4) {
                    lines.push(format!("  file: {}", patch.file));
                    lines.push(format!(
                        "  reasoning: {}",
                        preview_inline_chars(&patch.reasoning, 180)
                    ));
                    lines.push(format!(
                        "  diff: {}",
                        preview_inline_chars(&patch.diff, 240)
                    ));
                }
                if args.patches.len() > 4 {
                    lines.push(format!("  ... {} more patch(es)", args.patches.len() - 4));
                }
                lines.join("\n")
            }
            ToolCallArguments::Cargo(args) => compact_fields(vec![
                Some(format!("command: {:?}", args.command)),
                Some(format!("scope: {:?}", args.scope)),
                field_opt("package", args.package),
                field_opt_vec("features", args.features),
                field_opt("target", args.target),
                field_opt("profile", args.profile),
                Some(format!("release: {}", args.release)),
                Some(format!("include_warnings: {}", args.include_warnings)),
            ]),
            ToolCallArguments::ListDir(args) => compact_fields(vec![
                Some(format!("dir: {}", args.dir)),
                Some(format!("include_hidden: {}", args.include_hidden)),
                field_opt("sort", args.sort),
                field_opt_u32("max_entries", args.max_entries),
            ]),
            other => pretty_json_preview(&other, 1_200),
        },
        PersistedToolCallArguments::ParseFailure(failure) => {
            if let Some(arguments) = raw_json_fields(&failure.raw_arguments) {
                arguments
            } else {
                format!(
                    "arguments: {}\nparse_note: {:?}",
                    preview_inline_chars(&failure.raw_arguments, 500),
                    failure.error
                )
            }
        }
    }
}

fn render_tool_results(results: &[ToolLoopResult]) -> String {
    let mut lines = vec![format!("tool_results: {}", results.len())];
    if results.is_empty() {
        lines.push("  (none)".to_string());
        return lines.join("\n");
    }
    for (index, result) in results.iter().enumerate() {
        match result {
            ToolLoopResult::Completed(record) => {
                lines.push(format!(
                    "  {}. completed {} call_id={} latency_ms={}",
                    index + 1,
                    record.tool,
                    record.call_id,
                    record.latency_ms
                ));
                lines.extend(indent_lines(&format_completed_result(record), 5));
            }
            ToolLoopResult::Failed(record) => {
                lines.push(format!(
                    "  {}. failed {} call_id={} latency_ms={}",
                    index + 1,
                    record.tool.as_deref().unwrap_or("unknown"),
                    record.call_id,
                    record.latency_ms
                ));
                lines.extend(indent_lines(&format_failed_result(record), 5));
            }
        }
    }
    lines.join("\n")
}

fn format_completed_result(record: &ToolCompletedRecord) -> String {
    match decode_tool_result_content(&record.tool, &record.content) {
        PersistedToolResultContent::Decoded(result) => match result {
            ToolResultContent::ListDir(result) => {
                let mut lines = vec![format!(
                    "ok={} dir={} exists={} entries={} truncated={}",
                    result.ok,
                    result.dir,
                    result.exists,
                    result.entries.len(),
                    result.truncated
                )];
                for entry in result.entries.iter().take(8) {
                    lines.push(format!(
                        "- {} {} kind={} size={}",
                        entry.path,
                        entry.name,
                        entry.kind,
                        entry
                            .size_bytes
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    ));
                }
                if result.entries.len() > 8 {
                    lines.push(format!("... {} more entrie(s)", result.entries.len() - 8));
                }
                lines.join("\n")
            }
            ToolResultContent::Cargo(result) => {
                let mut lines = vec![format!(
                    "ok={} command={:?} scope={:?} status={:?} exit_code={} duration_ms={}",
                    result.ok,
                    result.command,
                    result.scope,
                    result.status_reason,
                    result
                        .exit_code
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    result.duration_ms
                )];
                lines.push(format!("manifest: {}", result.manifest_path));
                lines.push(format!(
                    "summary: errors={} warnings={} notes={} artifacts={} other_messages={}",
                    result.summary.errors,
                    result.summary.warnings,
                    result.summary.notes,
                    result.summary.artifacts,
                    result.summary.other_messages
                ));
                if !result.diagnostics.is_empty() {
                    lines.push("diagnostics:".to_string());
                    for diagnostic in result.diagnostics.iter().take(4) {
                        lines.push(format!(
                            "  - {} {}",
                            diagnostic.level,
                            preview_inline_chars(&diagnostic.message, 220)
                        ));
                    }
                }
                if !result.stderr_tail.is_empty() {
                    lines.push("stderr_tail:".to_string());
                    for line in result
                        .stderr_tail
                        .iter()
                        .rev()
                        .take(4)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                    {
                        lines.push(format!("  {}", preview_inline_chars(line, 220)));
                    }
                }
                lines.join("\n")
            }
            ToolResultContent::NsPatch(result) => compact_fields(vec![
                Some(format!("ok: {}", result.ok)),
                Some(format!("staged: {}", result.staged)),
                Some(format!("applied: {}", result.applied)),
                Some(format!("files: {}", result.files.join(", "))),
                Some(format!("preview_mode: {}", result.preview_mode)),
                Some(format!("auto_confirmed: {}", result.auto_confirmed)),
            ]),
            ToolResultContent::NsRead(result) => {
                let mut lines = vec![format!(
                    "ok={} file={} exists={} bytes={} lines={}-{} truncated={}",
                    result.ok,
                    result.file_path,
                    result.exists,
                    result
                        .byte_len
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    result
                        .start_line
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    result
                        .end_line
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    result.truncated
                )];
                if let Some(content) = result.content.as_deref() {
                    lines.extend(indent_lines(&text_block("content", content, 900), 0));
                }
                lines.join("\n")
            }
            other => pretty_json_preview(&other, 1_600),
        },
        PersistedToolResultContent::ParseFailure(failure) => format!(
            "raw_content: {}\nparse_error: {:?}",
            preview_inline_chars(&failure.raw_content, 1_000),
            failure.error
        ),
    }
}

fn format_failed_result(record: &ToolFailedRecord) -> String {
    if let Some(wire) = ToolErrorWire::parse(&record.error) {
        let mut lines = vec![format!("user: {}", preview_inline_chars(&wire.user, 700))];
        lines.push(format!(
            "llm: tool={} code={:?} field={} expected={} received={}",
            wire.llm.tool.as_str(),
            wire.llm.code,
            wire.llm.field.as_deref().unwrap_or("-"),
            wire.llm.expected.as_deref().unwrap_or("-"),
            wire.llm.received.as_deref().unwrap_or("-")
        ));
        lines.push(format!(
            "message: {}",
            preview_inline_chars(&wire.llm.message, 700)
        ));
        if let Some(hint) = wire.llm.retry_hint.as_deref() {
            lines.push(format!("retry_hint: {}", preview_inline_chars(hint, 900)));
        }
        if let Some(context) = wire.llm.retry_context.as_ref() {
            lines.push("retry_context:".to_string());
            for field in &context.fields {
                lines.push(format!(
                    "  {}: {}",
                    field.name,
                    retry_context_value_label(&field.value)
                ));
            }
        }
        lines.push(format!(
            "system: {}",
            preview_inline_chars(&wire.system, 700)
        ));
        lines.join("\n")
    } else {
        format!("error: {}", preview_inline_chars(&record.error, 1_200))
    }
}

fn render_llm_next_commands(
    lane: &LlmLane,
    record: &crate::replay::tool_loop::ToolLoopStep,
) -> Vec<String> {
    let mut lines = vec!["next:".to_string()];
    if !record.tool_requests.is_empty() {
        lines.push("  walk llm tool".to_string());
    }
    if record.step_index > 0 {
        lines.push("  walk llm back".to_string());
    }
    if lane.head.is_some_and(|head| record.step_index < head) {
        lines.push("  walk llm forward".to_string());
        lines.push("  walk llm head".to_string());
    }
    if !record.terminal {
        lines.push("  walk llm step --source live --watch --allow workspace-mutation".to_string());
    } else {
        lines.push(
            "  terminal inner frame; use walk llm back to inspect prior tool calls".to_string(),
        );
    }
    lines
}

fn compact_fields(fields: Vec<Option<String>>) -> String {
    let values = fields.into_iter().flatten().collect::<Vec<_>>();
    if values.is_empty() {
        "(none)".to_string()
    } else {
        values.join("\n")
    }
}

fn field_opt(name: &str, value: Option<String>) -> Option<String> {
    value.map(|value| format!("{name}: {value}"))
}

fn field_opt_u32(name: &str, value: Option<u32>) -> Option<String> {
    value.map(|value| format!("{name}: {value}"))
}

fn field_opt_vec(name: &str, value: Option<Vec<String>>) -> Option<String> {
    value.map(|value| format!("{name}: {}", value.join(", ")))
}

fn text_block(label: &str, text: &str, max_chars: usize) -> String {
    let text = truncate_chars(text.trim(), max_chars);
    if text.is_empty() {
        return format!("{label}: (empty)");
    }
    let mut lines = vec![format!("{label}:")];
    lines.extend(text.lines().map(|line| format!("  {line}")));
    lines.join("\n")
}

fn pretty_json_preview<T: serde::Serialize>(value: &T, max_chars: usize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(encoded) => truncate_chars(&encoded, max_chars),
        Err(error) => format!("<serialize error: {error}>"),
    }
}

fn raw_json_fields(raw: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    match value {
        serde_json::Value::Object(map) => {
            let mut lines = Vec::new();
            for (key, value) in map {
                lines.push(format!("{key}: {}", json_value_label(&value)));
            }
            Some(lines.join("\n"))
        }
        other => Some(pretty_json_preview(&other, 900)),
    }
}

fn json_value_label(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Array(values) => {
            let rendered = values
                .iter()
                .take(4)
                .map(json_value_label)
                .collect::<Vec<_>>()
                .join(", ");
            if values.len() > 4 {
                format!("[{rendered}, ...]")
            } else {
                format!("[{rendered}]")
            }
        }
        serde_json::Value::Object(_) => preview_inline_chars(&pretty_json_preview(value, 500), 500),
    }
}

fn retry_context_value_label(value: &ToolRetryContextValue) -> String {
    match value {
        ToolRetryContextValue::Null => "null".to_string(),
        ToolRetryContextValue::Bool(value) => value.to_string(),
        ToolRetryContextValue::Number(value) | ToolRetryContextValue::String(value) => {
            value.clone()
        }
        ToolRetryContextValue::StringList(values) => values.join(", "),
    }
}

fn preview_inline_chars(value: &str, max_chars: usize) -> String {
    let single_line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_chars(&single_line, max_chars)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
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
    use ploke_records::{
        agent_turn::{ToolCompletedRecord, ToolFailedRecord, ToolRequestRecord},
        llm_response::RawFullResponseRecord,
        tool_contracts::ToolArgumentsJson,
    };
    use uuid::Uuid;

    use crate::{
        cli::prototype1_state::identity::{ParentIdentity, write_parent_identity},
        replay::tool_loop::{
            FsToolLoopStore, ToolLoopResult, ToolLoopResume, ToolLoopSession, ToolLoopStep,
            ToolLoopStore, WorkspaceState,
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

    fn tool_call_response(index: usize) -> RawFullResponseRecord {
        let response = serde_json::from_value(serde_json::json!({
            "id": format!("chatcmpl-walk-llm-tool-{index}"),
            "choices": [{
                "index": 0,
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "content": "Calling tools...",
                    "tool_calls": [{
                        "id": "call-list-dir",
                        "type": "function",
                        "function": {
                            "name": "list_dir",
                            "arguments": "{\"dir\":\"crates/ploke-tree-browser\"}"
                        }
                    }]
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

    fn protected_write_error() -> String {
        serde_json::json!({
            "user": "non_semantic_patch: Write path denied before executing `non_semantic_patch`: Cargo.toml. Reason: path 'Cargo.toml' is protected",
            "llm": {
                "ok": false,
                "tool": "non_semantic_patch",
                "code": "invalid_format",
                "field": "patches.file",
                "expected": "workspace-root-relative path inside the writable surface",
                "received": "Cargo.toml",
                "message": "Write path denied before executing `non_semantic_patch`: Cargo.toml. Reason: path 'Cargo.toml' is protected",
                "snippet": null,
                "retry_hint": "Choose a workspace source file that is inside the writable surface. Do not edit protected manifests/configs such as Cargo.toml.",
                "retry_context": {
                    "fields": [{
                        "name": "input_paths",
                        "value": {"kind": "string_list", "value": ["Cargo.toml"]}
                    }]
                }
            },
            "system": "tool=NsPatch code=InvalidFormat: Write path denied before executing `non_semantic_patch`: Cargo.toml"
        })
        .to_string()
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

    fn dummy_lane() -> LlmLane {
        let mut session = ToolLoopSession::new("session-1", "headless-tui", PathBuf::from("work"));
        session.lane_id = Some("lane-1".to_string());
        LlmLane {
            lane_id: "lane-1".to_string(),
            session,
            resume: None,
            head: Some(0),
        }
    }

    #[test]
    fn llm_step_requires_explicit_workspace_mutation_gate() {
        let root = tempfile::tempdir().expect("tempdir");
        let controller = WalkController::new(root.path().join("repo"));
        let store = FsToolLoopStore::new(root.path().join("tool-loop"));

        let result = controller.prepare_llm_step(
            &store,
            &dummy_lane(),
            None,
            Prototype1StateWalkLlmStepSource::Historical,
            false,
            false,
            None,
            None,
            1,
            30,
        );
        let err = match result {
            Ok(_) => panic!("missing mutation gate should be rejected before any execution"),
            Err(err) => err,
        };

        assert!(
            err.to_string().contains("--allow workspace-mutation"),
            "error should explain the required effectful gate: {err}"
        );
    }

    #[test]
    fn live_llm_step_requires_watch_gate() {
        let root = tempfile::tempdir().expect("tempdir");
        let controller = WalkController::new(root.path().join("repo"));
        let store = FsToolLoopStore::new(root.path().join("tool-loop"));

        let result = controller.prepare_llm_step(
            &store,
            &dummy_lane(),
            None,
            Prototype1StateWalkLlmStepSource::Live,
            false,
            true,
            None,
            None,
            1,
            30,
        );
        let err = match result {
            Ok(_) => panic!("live provider stepping should require --watch"),
            Err(err) => err,
        };

        assert!(
            err.to_string().contains("--watch"),
            "error should explain that live stepping calls the provider: {err}"
        );
    }

    #[test]
    fn provider_override_requires_model_override() {
        let err = resolve_llm_model(None, None, Some("google"))
            .expect_err("provider without model cannot identify a routed live step");

        assert!(
            err.to_string().contains("requires --model-id"),
            "error should name the missing model override: {err}"
        );
    }

    #[test]
    fn llm_step_summary_points_to_new_session_head() {
        let session_id = uuid::Uuid::new_v4();
        let summary = LlmStepSummary {
            lane_id: "lane-1".to_string(),
            source: Prototype1StateWalkLlmStepSource::Historical,
            selected_step: 3,
            session_id,
            outcome: "completed".to_string(),
            attempts: 1,
            final_messages: 7,
            head: Some(0),
            terminal: true,
        };

        let rendered = summary.render();
        assert!(rendered.contains("source: Historical"));
        assert!(rendered.contains("selected_step: 3"));
        assert!(
            rendered.contains("inspect: walk llm show"),
            "summary should give the operator a focused follow-up inspection command: {rendered}"
        );
        assert!(
            rendered.contains(&format!(
                "inspect_explicit: walk llm show --session-id {session_id} --head"
            )),
            "summary should also keep an explicit session fallback: {rendered}"
        );
    }

    #[test]
    fn effectful_llm_commands_focus_new_session_head() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsToolLoopStore::new(tmp.path().join("tool-loop"));
        let session =
            ToolLoopSession::new("branched-session", "headless-tui", tmp.path().join("work"));
        store.write_session(&session).expect("session");
        let step = ToolLoopStep::new(
            "branched-session",
            3,
            vec![RequestMessage::new_user("continue".to_string())],
            content_response(3),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("step");
        store.write_step(&step).expect("step file");

        let mut controller = WalkController::new(tmp.path().join("repo"));
        controller
            .focus_llm_session_head(&store, "branched-session")
            .expect("focus branched session");

        assert_eq!(controller.llm_focus.as_deref(), Some("branched-session"));
        assert_eq!(controller.llm_cursors.get("branched-session"), Some(&3));
    }

    #[test]
    fn llm_timeline_summarizes_steps_and_marks_cursor() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsToolLoopStore::new(tmp.path().join("tool-loop"));
        let mut session =
            ToolLoopSession::new("session-timeline", "headless-tui", tmp.path().join("work"));
        session.lane_id = Some("lane-a".to_string());
        store.write_session(&session).expect("session");

        let mut listed = ToolLoopStep::new(
            "session-timeline",
            0,
            vec![RequestMessage::new_user("inspect".to_string())],
            tool_call_response(0),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("list step");
        listed.tool_requests.push(ToolRequestRecord {
            request_id: "session-timeline:0".to_string(),
            parent_id: "parent".to_string(),
            call_id: "call-list-dir".to_string(),
            tool: "list_dir".to_string(),
            arguments: ToolArgumentsJson::from(r#"{"dir":"crates/ploke-tree-browser"}"#),
        });
        listed.tool_results.push(ToolLoopResult::Completed(ToolCompletedRecord {
            request_id: "session-timeline:0".to_string(),
            parent_id: "parent".to_string(),
            call_id: "call-list-dir".to_string(),
            tool: "list_dir".to_string(),
            content: serde_json::json!({
                "ok": true,
                "dir": "crates/ploke-tree-browser",
                "exists": true,
                "truncated": false,
                "entries": [
                    {"name":"Cargo.toml","path":"crates/ploke-tree-browser/Cargo.toml","kind":"file","size_bytes":333,"modified_ms":null},
                    {"name":"src","path":"crates/ploke-tree-browser/src","kind":"dir","size_bytes":null,"modified_ms":null}
                ]
            })
            .to_string(),
            ui_payload: None,
            latency_ms: 0,
        }));
        store.write_step(&listed).expect("list step file");

        let mut denied = ToolLoopStep::new(
            "session-timeline",
            1,
            vec![RequestMessage::new_user("patch".to_string())],
            tool_call_response(1),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("denied step");
        denied.tool_requests.push(ToolRequestRecord {
            request_id: "session-timeline:1".to_string(),
            parent_id: "parent".to_string(),
            call_id: "call-patch".to_string(),
            tool: "non_semantic_patch".to_string(),
            arguments: ToolArgumentsJson::from(r#"{"patches":[{"file":"Cargo.toml","diff":"--- a/Cargo.toml","reasoning":"remove crate"}]}"#),
        });
        denied
            .tool_results
            .push(ToolLoopResult::Failed(ToolFailedRecord {
                request_id: "session-timeline:1".to_string(),
                parent_id: "parent".to_string(),
                call_id: "call-patch".to_string(),
                tool: Some("non_semantic_patch".to_string()),
                error: protected_write_error(),
                ui_payload: None,
                latency_ms: 0,
            }));
        store.write_step(&denied).expect("denied step file");

        let mut terminal = ToolLoopStep::new(
            "session-timeline",
            2,
            vec![RequestMessage::new_user("finish".to_string())],
            content_response(2),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("terminal step");
        terminal.terminal = true;
        store.write_step(&terminal).expect("terminal step file");

        let lane = LlmLane {
            lane_id: "lane-a".to_string(),
            session,
            resume: None,
            head: Some(2),
        };
        let mut controller = WalkController::new(tmp.path().join("repo"));
        controller
            .llm_cursors
            .insert("session-timeline".to_string(), 1);

        let rendered = controller
            .render_llm_timeline(&store, lane)
            .expect("timeline render");

        assert!(rendered.contains("llm tool-loop timeline"));
        assert!(rendered.contains("steps: 3"));
        assert!(rendered.contains("#0000 tool"));
        assert!(rendered.contains("list_dir crates/ploke-tree-browser"));
        assert!(rendered.contains("ok entries=2"));
        assert!(rendered.contains("* #0001 tool"));
        assert!(rendered.contains("non_semantic_patch Cargo.toml"));
        assert!(rendered.contains("failed protected-write"));
        assert!(rendered.contains("#0002 content"));
        assert!(rendered.contains("terminal"));
        assert!(rendered.contains("walk llm show"));
    }

    #[test]
    fn llm_checkpoint_render_shows_tool_arguments_and_decoded_results() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsToolLoopStore::new(tmp.path().join("tool-loop"));
        let mut session =
            ToolLoopSession::new("session-tool", "headless-tui", tmp.path().join("lane-a"));
        session.lane_id = Some("lane-a".to_string());
        let lane = LlmLane {
            lane_id: "lane-a".to_string(),
            session,
            resume: None,
            head: Some(2),
        };
        let mut step = ToolLoopStep::new(
            "session-tool",
            2,
            vec![
                RequestMessage::new_user("inspect the tree browser crate".to_string()),
                RequestMessage::new_assistant("I will inspect the crate.".to_string()),
            ],
            tool_call_response(2),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("step");
        step.tool_requests.push(ToolRequestRecord {
            request_id: "session-tool:2".to_string(),
            parent_id: "parent".to_string(),
            call_id: "call-list-dir".to_string(),
            tool: "list_dir".to_string(),
            arguments: ToolArgumentsJson::from(r#"{"dir":"crates/ploke-tree-browser"}"#),
        });
        step.tool_results.push(ToolLoopResult::Completed(ToolCompletedRecord {
            request_id: "session-tool:2".to_string(),
            parent_id: "parent".to_string(),
            call_id: "call-list-dir".to_string(),
            tool: "list_dir".to_string(),
            content: serde_json::json!({
                "ok": true,
                "dir": "crates/ploke-tree-browser",
                "exists": true,
                "truncated": false,
                "entries": [
                    {"name":"Cargo.toml","path":"crates/ploke-tree-browser/Cargo.toml","kind":"file","size_bytes":333,"modified_ms":null},
                    {"name":"src","path":"crates/ploke-tree-browser/src","kind":"dir","size_bytes":null,"modified_ms":null}
                ]
            })
            .to_string(),
            ui_payload: None,
            latency_ms: 0,
        }));

        let rendered = WalkController::new(tmp.path().join("repo")).render_llm_checkpoint(
            &store,
            lane,
            Some((2, step)),
        );

        assert!(rendered.contains("assistant_response:"));
        assert!(rendered.contains("content:"));
        assert!(rendered.contains("Calling tools..."));
        assert!(rendered.contains("tool_calls: 1"));
        assert!(rendered.contains("list_dir call_id=call-list-dir"));
        assert!(rendered.contains("dir: crates/ploke-tree-browser"));
        assert!(rendered.contains("tool_results: 1"));
        assert!(rendered.contains("completed list_dir"));
        assert!(rendered.contains("entries=2"));
        assert!(rendered.contains("crates/ploke-tree-browser/Cargo.toml"));
        assert!(rendered.contains("prior_messages:"));
        assert!(rendered.contains("next:"));
    }

    #[test]
    fn llm_tool_render_shows_definition_arguments_and_json() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut session =
            ToolLoopSession::new("session-tool", "headless-tui", tmp.path().join("lane-a"));
        session.lane_id = Some("lane-a".to_string());
        let lane = LlmLane {
            lane_id: "lane-a".to_string(),
            session: session.clone(),
            resume: None,
            head: Some(2),
        };
        let mut step = ToolLoopStep::new(
            "session-tool",
            2,
            vec![RequestMessage::new_user("inspect".to_string())],
            tool_call_response(2),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("step");
        step.tool_requests.push(ToolRequestRecord {
            request_id: "session-tool:2".to_string(),
            parent_id: "parent".to_string(),
            call_id: "call-list-dir".to_string(),
            tool: "list_dir".to_string(),
            arguments: ToolArgumentsJson::from(r#"{"dir":"crates/ploke-tree-browser"}"#),
        });

        let controller = WalkController::new(tmp.path().join("repo"));
        let rendered = controller
            .render_llm_tool(lane.clone(), Some((2, step.clone())), None, None, false)
            .expect("human tool render");

        assert!(rendered.contains("llm tool"));
        assert!(rendered.contains("tool: list_dir"));
        assert!(rendered.contains("definition_source: current_renderer_checkout"));
        assert!(rendered.contains("arguments_source: persisted_checkpoint"));
        assert!(rendered.contains("arguments_json:"));
        assert!(rendered.contains("\"dir\": \"crates/ploke-tree-browser\""));
        assert!(rendered.contains("description:"));
        assert!(rendered.contains("parameters:"));
        assert!(rendered.contains("dir: string required"));

        let json = controller
            .render_llm_tool(lane, Some((2, step)), None, None, true)
            .expect("json tool render");
        let value: serde_json::Value = serde_json::from_str(&json).expect("tool json");
        assert_eq!(value["tool"], "list_dir");
        assert_eq!(value["call_id"], "call-list-dir");
        assert_eq!(value["arguments"]["dir"], "crates/ploke-tree-browser");
        assert_eq!(value["tool_definition"]["function"]["name"], "list_dir");
    }

    #[test]
    fn llm_checkpoint_render_shows_protected_write_retry_hint() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsToolLoopStore::new(tmp.path().join("tool-loop"));
        let mut session =
            ToolLoopSession::new("session-failed", "headless-tui", tmp.path().join("lane-a"));
        session.lane_id = Some("lane-a".to_string());
        let lane = LlmLane {
            lane_id: "lane-a".to_string(),
            session,
            resume: None,
            head: Some(11),
        };
        let mut step = ToolLoopStep::new(
            "session-failed",
            11,
            vec![RequestMessage::new_user("try a patch".to_string())],
            tool_call_response(11),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("step");
        step.tool_requests.push(ToolRequestRecord {
            request_id: "session-failed:11".to_string(),
            parent_id: "parent".to_string(),
            call_id: "call-patch".to_string(),
            tool: "non_semantic_patch".to_string(),
            arguments: ToolArgumentsJson::from(r#"{"patches":[{"file":"Cargo.toml","diff":"--- a/Cargo.toml","reasoning":"remove crate"}]}"#),
        });
        step.tool_results
            .push(ToolLoopResult::Failed(ToolFailedRecord {
                request_id: "session-failed:11".to_string(),
                parent_id: "parent".to_string(),
                call_id: "call-patch".to_string(),
                tool: Some("non_semantic_patch".to_string()),
                error: protected_write_error(),
                ui_payload: None,
                latency_ms: 0,
            }));

        let rendered = WalkController::new(tmp.path().join("repo")).render_llm_checkpoint(
            &store,
            lane,
            Some((11, step)),
        );

        assert!(rendered.contains("failed non_semantic_patch"));
        assert!(rendered.contains("file: Cargo.toml"));
        assert!(rendered.contains("retry_hint:"));
        assert!(rendered.contains("Do not edit protected manifests/configs"));
        assert!(rendered.contains("retry_context:"));
        assert!(rendered.contains("input_paths: Cargo.toml"));
        assert!(
            rendered.contains("walk llm step --source live --watch --allow workspace-mutation")
        );
    }

    #[test]
    fn reconstruction_note_abbreviates_parent_identity_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path().join("repo");
        let mut files = WalkFiles::default();
        files.reset(&repo);
        let note = format!(
            "parent identity: {} (node=node-1, generation=0, branch=branch-1)",
            parent_identity_path(&repo).display()
        );

        let rendered = render_reconstruction_note(&note, &files);

        assert!(
            rendered.contains("parent_identity: {root}/.ploke/prototype1/parent_identity.json")
        );
        assert!(rendered.contains("node=node-1, generation=0, branch=branch-1"));
        assert!(!rendered.contains(tmp.path().to_str().expect("temp path")));
    }

    #[test]
    fn typestate_changes_pretty_print_long_evidence_axis() {
        let mut lines = Vec::new();

        push_changes(
            &mut lines,
            WalkPhase::R13a,
            WalkPhase::R14a,
            "typestate changes",
        );
        let rendered = lines.join("\n");

        assert!(rendered.contains("  - evidence:"));
        assert!(rendered.contains("    Evidence<"));
        assert!(rendered.contains("    -> Evidence<"));
        assert!(rendered.contains("        evidence::completion::Recorded,"));
        assert!(!rendered.contains(
            "Evidence<evidence::parent_start::Recorded<ParentStartedEntry>, evidence::baseline"
        ));
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
