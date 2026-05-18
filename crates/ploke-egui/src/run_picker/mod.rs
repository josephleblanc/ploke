//! Filesystem-backed run picker for the native operator UI.
//!
//! This module discovers candidate run roots and derives compact UI summaries
//! through the existing typed import path. It does not parse run records
//! directly and does not create graph semantics.

#[cfg(feature = "dev")]
mod artifact_connectivity;
mod diagnostics;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use eframe::egui;
use ploke_tree::{FsRunStore, Graph, RunRootSummary};

use crate::ui::app::layout;

#[cfg(feature = "dev")]
pub use artifact_connectivity::{
    ArtifactConnectivityBatch, ArtifactConnectivityEntry, ArtifactConnectivityError,
};
pub use diagnostics::{EntryDiagnostics, PickerDiagnostics};

const RUN_NAME_MAX_CHARS: usize = 28;

#[derive(Debug)]
pub struct RunPicker {
    root: PathBuf,
    entries: Vec<RunEntry>,
    selected: Option<usize>,
    last_error: Option<String>,
}

impl RunPicker {
    pub fn from_default_root() -> Self {
        let root =
            default_campaigns_root().unwrap_or_else(|| PathBuf::from("~/.ploke-eval/campaigns"));
        Self::from_root(root)
    }

    pub fn from_default_root_deferred() -> Self {
        let root =
            default_campaigns_root().unwrap_or_else(|| PathBuf::from("~/.ploke-eval/campaigns"));
        Self::from_root_deferred(root)
    }

    pub fn from_root(root: PathBuf) -> Self {
        let mut picker = Self {
            root,
            entries: Vec::new(),
            selected: None,
            last_error: None,
        };
        picker.refresh();
        picker
    }

    pub fn from_root_deferred(root: PathBuf) -> Self {
        Self {
            root,
            entries: Vec::new(),
            selected: None,
            last_error: None,
        }
    }

    pub fn first_loadable_path(&self) -> Option<&Path> {
        self.entries
            .iter()
            .find(|entry| entry.summary.is_some())
            .map(|entry| entry.path.as_path())
    }

    pub fn select_path(&mut self, path: &Path) {
        self.selected = self
            .entries
            .iter()
            .position(|entry| same_path(entry.path.as_path(), path));
    }

    pub fn select_or_insert_path(&mut self, path: &Path) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| same_path(entry.path.as_path(), path))
        {
            self.selected = Some(index);
            return;
        }

        let entry = RunEntry::new(
            run_name(path),
            path.to_path_buf(),
            path.metadata()
                .and_then(|metadata| metadata.modified())
                .ok(),
            None,
            None,
        );
        self.entries.insert(0, entry);
        self.selected = Some(0);
    }

    pub fn record_loaded_graph(&mut self, path: &Path, graph: &Graph) {
        self.select_or_insert_path(path);
        if let Some(index) = self.selected
            && let Some(entry) = self.entries.get_mut(index)
        {
            entry.set_summary(Some(RunSummary::from_graph(graph)), None);
        }
    }

    pub fn selected_run(&self) -> Option<SelectedRun> {
        self.selected_entry().map(|entry| SelectedRun {
            name: entry.name.clone(),
            path: entry.path.clone(),
        })
    }

    pub fn selected_run_name(&self) -> Option<&str> {
        self.selected_entry().map(|entry| entry.name.as_str())
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<PathBuf> {
        let mut selected_path = None;

        ui.horizontal(|ui| {
            ui.label("Run");
            if ui.button("Refresh").clicked() {
                self.refresh();
            }
        });

        let selected_text = self
            .selected
            .and_then(|index| self.entries.get(index))
            .map(RunEntry::selected_label)
            .unwrap_or("Select run");

        egui::ComboBox::from_id_salt("ploke-egui.run-picker")
            .width(ui.available_width().min(layout::LEFT_SIDEBAR_WIDTH))
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for (index, entry) in self.entries.iter().enumerate() {
                    if ui
                        .selectable_label(self.selected == Some(index), entry.menu_label())
                        .clicked()
                    {
                        selected_path = Some(entry.path.clone());
                    }
                }
            });

        if let Some(path) = selected_path.as_deref() {
            self.select_path(path);
        }

        if let Some(entry) = self.selected_entry() {
            entry.show_summary(ui);
        }

        if self.entries.is_empty() {
            ui.label(format!(
                "No run directories found in {}",
                self.root.display()
            ));
        }

        if let Some(error) = &self.last_error {
            ui.label(error);
        }

        selected_path
    }

    fn selected_entry(&self) -> Option<&RunEntry> {
        self.selected.and_then(|index| self.entries.get(index))
    }

    fn refresh(&mut self) {
        match discover_runs(&self.root) {
            Ok(entries) => {
                let selected_path = self
                    .selected
                    .and_then(|index| self.entries.get(index))
                    .map(|entry| entry.path.clone());
                self.entries = entries;
                self.selected = selected_path.as_deref().and_then(|path| {
                    self.entries
                        .iter()
                        .position(|entry| same_path(&entry.path, path))
                });
                self.last_error = None;
            }
            Err(error) => {
                self.entries.clear();
                self.selected = None;
                self.last_error = Some(format!("Run discovery failed: {error}"));
            }
        }
    }
}

impl Default for RunPicker {
    fn default() -> Self {
        Self::from_default_root()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedRun {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
struct RunEntry {
    name: String,
    path: PathBuf,
    modified: Option<SystemTime>,
    summary: Option<RunSummary>,
    error: Option<String>,
    selected_label: String,
    menu_label: String,
}

impl RunEntry {
    fn new(
        name: String,
        path: PathBuf,
        modified: Option<SystemTime>,
        summary: Option<RunSummary>,
        error: Option<String>,
    ) -> Self {
        let selected_label = elide_middle(&name, RUN_NAME_MAX_CHARS);
        let menu_label = match &summary {
            Some(summary) => format!("{}  |  {}", name, summary.compact()),
            None => format!("{name}  |  unreadable"),
        };
        Self {
            name,
            path,
            modified,
            summary,
            error,
            selected_label,
            menu_label,
        }
    }

    fn selected_label(&self) -> &str {
        self.selected_label.as_str()
    }

    fn menu_label(&self) -> &str {
        self.menu_label.as_str()
    }

    fn set_summary(&mut self, summary: Option<RunSummary>, error: Option<String>) {
        self.summary = summary;
        self.error = error;
        self.menu_label = match &self.summary {
            Some(summary) => format!("{}  |  {}", self.name, summary.compact()),
            None => format!("{}  |  unreadable", self.name),
        };
    }

    fn show_summary(&self, ui: &mut egui::Ui) {
        match &self.summary {
            Some(summary) => {
                ui.label(summary.primary_line());
                ui.label(summary.secondary_line());
            }
            None => {
                let response = ui.label("unreadable");
                if let Some(error) = &self.error {
                    response.on_hover_text(error);
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct RunSummary {
    primary_line: String,
    secondary_line: String,
    compact: String,
}

impl RunSummary {
    fn from_graph(graph: &Graph) -> Self {
        Self::from_counts(
            graph.artifacts.artifacts.len(),
            graph.history.blocks.len(),
            graph.candidates.candidates.len(),
        )
    }

    fn from_run_root_summary(summary: &RunRootSummary) -> Self {
        Self::from_counts(
            summary.artifact_count,
            summary.history_block_count,
            summary.candidate_count,
        )
    }

    fn from_counts(artifacts: usize, history_blocks: usize, candidates: usize) -> Self {
        Self {
            primary_line: format!("{artifacts} artifacts"),
            secondary_line: format!("{history_blocks} history | {candidates} candidates"),
            compact: format!(
                "{artifacts} artifacts, {history_blocks} history, {candidates} candidates"
            ),
        }
    }

    fn primary_line(&self) -> &str {
        self.primary_line.as_str()
    }

    fn secondary_line(&self) -> &str {
        self.secondary_line.as_str()
    }

    fn compact(&self) -> &str {
        self.compact.as_str()
    }
}

fn discover_runs(root: &Path) -> Result<Vec<RunEntry>, std::io::Error> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() {
            continue;
        }

        let campaign_path = entry.path();
        let path = campaign_path.join("prototype1");
        if !path.is_dir() {
            continue;
        }
        let name = entry
            .file_name()
            .to_str()
            .map(str::to_owned)
            .unwrap_or_else(|| campaign_path.display().to_string());
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok();
        let (summary, error) = match FsRunStore::new(&path).load_run_root_summary() {
            Ok(summary) => (Some(RunSummary::from_run_root_summary(&summary)), None),
            Err(error) => (None, Some(error.to_string())),
        };

        entries.push(RunEntry::new(name, path, modified, summary, error));
    }

    entries.sort_by(|left, right| {
        right
            .modified
            .cmp(&left.modified)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(entries)
}

fn run_name(path: &Path) -> String {
    path.parent()
        .and_then(Path::file_name)
        .or_else(|| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn default_campaigns_root() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".ploke-eval/campaigns"))
}

fn same_path(left: &Path, right: &Path) -> bool {
    left == right || normalized_path(left) == normalized_path(right)
}

fn normalized_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn elide_middle(value: &str, max_chars: usize) -> String {
    let len = value.chars().count();
    if len <= max_chars || max_chars < 5 {
        return value.to_owned();
    }

    let prefix_len = (max_chars - 1) / 2;
    let suffix_len = max_chars - prefix_len - 1;
    let prefix: String = value.chars().take(prefix_len).collect();
    let suffix: String = value
        .chars()
        .rev()
        .take(suffix_len)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}.{suffix}")
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn discovery_uses_campaign_prototype1_roots_not_worktrees()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "ploke-egui-run-picker-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        let campaign = root.join("campaign-a");
        let run_root = campaign.join("prototype1");
        let worktree_like = root.join("worktree-looking-dir");
        fs::create_dir_all(&run_root)?;
        fs::create_dir_all(&worktree_like)?;

        let entries = discover_runs(&root)?;

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "campaign-a");
        assert_eq!(entries[0].path, run_root);

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn discovery_uses_lightweight_summary_not_full_graph_import()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "ploke-egui-run-picker-summary-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        let run_root = root.join("campaign-a").join("prototype1");
        fs::create_dir_all(run_root.join("evaluations"))?;
        fs::write(
            run_root.join("scheduler.json"),
            r#"{"schema_version":"prototype1-scheduler.v1","campaign_id":"campaign-a","updated_at":"2026-05-18T00:00:00Z","nodes":[]}"#,
        )?;
        fs::write(run_root.join("evaluations").join("bad.json"), "{")?;

        let entries = discover_runs(&root)?;

        assert_eq!(entries.len(), 1);
        assert!(entries[0].summary.is_some());
        assert_eq!(entries[0].error, None);

        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn selected_label_keeps_sidebar_text_compact() {
        let entry = RunEntry::new(
            "p1-smoke-3x4-edit-surface-20260510-4".to_owned(),
            PathBuf::from("prototype1"),
            None,
            Some(RunSummary::from_counts(26, 5, 0)),
            None,
        );

        assert_eq!(entry.selected_label().chars().count(), RUN_NAME_MAX_CHARS);
        assert!(!entry.selected_label().contains("artifacts"));
        assert!(entry.menu_label().contains("26 artifacts"));
    }

    #[test]
    fn unreadable_menu_label_excludes_full_error() {
        let entry = RunEntry::new(
            "p1-error-run".to_owned(),
            PathBuf::from("prototype1"),
            None,
            None,
            Some(
                "failed to parse /very/long/path/node.json: expected struct NodeRecord".to_owned(),
            ),
        );

        assert_eq!(entry.menu_label(), "p1-error-run  |  unreadable");
        assert!(!entry.menu_label().contains("/very/long/path"));
        assert!(!entry.menu_label().contains("expected struct NodeRecord"));
    }

    #[test]
    fn prototype1_run_roots_do_not_match_by_leaf_name_only() {
        let left = Path::new("/tmp/campaign-a/prototype1");
        let right = Path::new("/tmp/campaign-b/prototype1");

        assert!(!same_path(left, right));
    }
}
