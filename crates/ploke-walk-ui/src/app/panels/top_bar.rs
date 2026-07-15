use eframe::egui::{Color32, ComboBox, Response, RichText, TextEdit, Ui, Widget};
use ploke_eval::walk_client::WalkRunEntry;

use crate::model::{RUN_LABEL_MAX_CHARS, ServiceStatus};

pub(in crate::app) struct TopBar<'a> {
    pub(in crate::app) status: &'a ServiceStatus,
    pub(in crate::app) runs: &'a [WalkRunEntry],
    pub(in crate::app) selected_run: Option<usize>,
    pub(in crate::app) socket_input: &'a mut String,
    pub(in crate::app) debug_panel: &'a mut bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub(in crate::app) struct TopBarAction {
    pub(in crate::app) selected_run: Option<usize>,
    pub(in crate::app) refresh_runs: bool,
    pub(in crate::app) socket_changed: bool,
}

impl TopBar<'_> {
    pub(in crate::app) fn show(self, ui: &mut Ui) -> TopBarAction {
        let mut action = TopBarAction::default();
        ui.horizontal_wrapped(|ui| {
            ui.add(StatusBadge {
                status: self.status,
            });
            if let ServiceStatus::Error(error) = self.status {
                ui.label(RichText::new(error).color(Color32::LIGHT_RED));
            }
            ui.separator();
            ui.label("run");
            let mut selected = self.selected_run;
            ComboBox::from_id_salt("ploke-walk-ui.run-picker")
                .width(440.0)
                .selected_text(selected_run_label(
                    self.selected_run.and_then(|index| self.runs.get(index)),
                ))
                .show_ui(ui, |ui| {
                    for (index, run) in self.runs.iter().enumerate() {
                        ui.selectable_value(&mut selected, Some(index), run_menu_label(run));
                    }
                });
            if selected != self.selected_run
                && let Some(index) = selected
            {
                action.selected_run = Some(index);
            }
            if ui.button("Refresh Runs").clicked() {
                action.refresh_runs = true;
            }
            ui.separator();
            ui.label("socket override");
            action.socket_changed = ui
                .add_sized([260.0, 22.0], TextEdit::singleline(self.socket_input))
                .changed();
            ui.toggle_value(self.debug_panel, "Debug");
        });
        action
    }
}

struct StatusBadge<'a> {
    status: &'a ServiceStatus,
}

impl Widget for StatusBadge<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let text = match self.status {
            ServiceStatus::Unresolved => RichText::new("unresolved").color(Color32::GRAY),
            ServiceStatus::Offline => RichText::new("offline").color(Color32::YELLOW),
            ServiceStatus::Online => RichText::new("online").color(Color32::GREEN),
            ServiceStatus::Busy(_) => RichText::new("busy").color(Color32::YELLOW),
            ServiceStatus::Error(_) => RichText::new("error").color(Color32::RED),
        };
        ui.label(text)
    }
}

fn selected_run_label(run: Option<&WalkRunEntry>) -> String {
    run.map(|run| elide_middle(&run.campaign_id, RUN_LABEL_MAX_CHARS))
        .unwrap_or_else(|| "Select run".to_string())
}

fn run_menu_label(run: &WalkRunEntry) -> String {
    let mut flags = Vec::new();
    flags.push(if run.has_owner_db { "db" } else { "no db" });
    flags.push(if run.worktree_root.is_some() {
        "worktree"
    } else {
        "no worktree"
    });
    flags.push(if run.has_parent_identity {
        "identity"
    } else {
        "no identity"
    });
    format!(
        "{}  |  {}",
        elide_middle(&run.campaign_id, RUN_LABEL_MAX_CHARS),
        flags.join(", ")
    )
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
