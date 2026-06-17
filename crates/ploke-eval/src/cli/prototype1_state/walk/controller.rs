//! In-memory typestate walker used by the local debug server.
//!
//! `WalkController` owns exactly one `WalkState` value. Each step consumes that
//! state and calls the canonical direct edge functions from `live_edges`; this
//! module should not duplicate transition semantics.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use ploke_records::ids::CampaignId;

use crate::{
    ResolvedCampaignConfig,
    cli::prototype1_state::{
        cli_facing::{Prototype1StateRunShape, campaign_manifest_path_for_id},
        identity::parent_identity_path,
        journal::prototype1_transition_journal_path,
        live_edges::{
            r0_to_r1, r1_to_r2a_or_r3, r3_to_r4a, r4a_to_r4b_or_r4c, r4b_to_r4c_genesis, r4c_to_r5,
        },
        typestate::{self, R0, R1, R2a, R3, R4a, R4bGenesisChecked, R4cReady, R5, StepInput},
    },
    layout::prototype1_monitor_target_file,
    spec::PrepareError,
};

use super::{paths, phase::WalkPhase, protocol::WalkStartConfig};

const MAX_HISTORY: usize = 80;
const MAX_FILE_BYTES: usize = 128 * 1024;

type RunShape = Prototype1StateRunShape;
type CampaignConfig = ResolvedCampaignConfig;

/// Single-session in-memory controller for early Prototype 1 typestate phases.
///
/// The current server slice admits setup/startup and parent-start phases through
/// `R5` so the socket lifecycle can be tested before exposing child fanout or
/// handoff.
pub(crate) struct WalkController {
    state: WalkState,
    steps: usize,
    previous: WalkPrevious,
    files: WalkFiles,
    last_delta: Option<WalkAdvanceReport>,
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
    pub(crate) fn new() -> Self {
        Self {
            state: WalkState::Empty,
            steps: 0,
            previous: WalkPrevious::default(),
            files: WalkFiles::default(),
            last_delta: None,
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

    /// Render the last successful step delta, if any.
    pub(crate) fn delta_report(&self, style: DeltaRenderStyle) -> String {
        self.last_delta
            .as_ref()
            .map(|delta| delta.render_delta(style))
            .unwrap_or_else(|| "no previous step delta; run `walk step` first".to_string())
    }

    /// Reset the current in-memory walk without stopping the server process.
    pub(crate) fn reset(&mut self) -> WalkPhase {
        let previous = self.phase();
        self.state = WalkState::Empty;
        self.steps = 0;
        self.files.clear();
        self.last_delta = None;
        self.record(format!("reset: cleared in-memory walk from {previous}"));
        self.phase()
    }

    /// Start a new walk from `R0` and optionally advance to an admitted boundary.
    pub(crate) fn start(
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
        self.files.reset(&repo_root);
        self.state = WalkState::R0(typestate::R0::new(config.into_state_command()));
        self.steps = 0;
        self.last_delta = None;
        self.record(format!(
            "start: created r0 for repo_root '{}'",
            repo_root.display()
        ));
        self.advance_until(until)?;
        Ok(self.phase())
    }

    /// Advance the current walk by one edge or until a requested phase.
    pub(crate) fn step(
        &mut self,
        until: Option<WalkPhase>,
    ) -> Result<WalkAdvanceReport, PrepareError> {
        let from = self.phase();
        let target = until.unwrap_or_else(|| self.phase().next().unwrap_or(self.phase()));
        ensure_supported_target(target)?;
        let transitions = if self.phase() == target {
            Vec::new()
        } else if until.is_some() {
            self.advance_until(target)?
        } else {
            vec![self.step_once()?]
        };
        let report = WalkAdvanceReport {
            from,
            to: self.phase(),
            transitions,
        };
        self.last_delta = Some(report.clone());
        Ok(report)
    }

    fn advance_until(&mut self, target: WalkPhase) -> Result<Vec<WalkTransition>, PrepareError> {
        let mut guard = 0_u8;
        let mut transitions = Vec::new();
        while self.phase() != target {
            guard += 1;
            if guard > 16 {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!("walk exceeded early-step guard while advancing to {target}"),
                });
            }
            transitions.push(self.step_once()?);
        }
        Ok(transitions)
    }

    fn step_once(&mut self) -> Result<WalkTransition, PrepareError> {
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
            WalkState::R5(r5) => {
                self.state = WalkState::R5(r5);
                let detail = "walk reached R5 parent-start boundary; R6+ phases are not admitted by this debug server slice yet";
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
                self.record(format!("step {}: {previous} -> {current}", self.steps));
                Ok(WalkTransition {
                    from: previous,
                    to: current,
                })
            }
            Err(error) => {
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
            WalkState::Failed { phase, .. } => *phase,
        }
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
            WalkPhase::R5 => None,
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
