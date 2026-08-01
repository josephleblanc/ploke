use eframe::egui::{
    self, Color32, Grid, Response, RichText, ScrollArea, TextEdit, TextStyle, Ui, Widget,
};
use ploke_eval::walk_client::{DbQueryResult, WalkEvidenceQuery, WalkQuerySnapshot};

use crate::model::{DEFAULT_QUERY, MAX_TABLE_ROWS};

pub(in crate::app) struct QueryPanel<'a> {
    pub(in crate::app) campaign_input: &'a mut String,
    pub(in crate::app) query_script: &'a mut String,
    pub(in crate::app) query_pending: bool,
    pub(in crate::app) query_result: Option<&'a WalkQuerySnapshot>,
    pub(in crate::app) selected_row: &'a mut Option<usize>,
    pub(in crate::app) selected_campaign: Option<&'a str>,
    pub(in crate::app) client_available: bool,
}

#[derive(Debug, Default)]
pub(in crate::app) struct QueryAction {
    pub(in crate::app) run_raw: bool,
    pub(in crate::app) view: Option<WalkEvidenceQuery>,
}

impl QueryPanel<'_> {
    pub(in crate::app) fn show(self, ui: &mut Ui) -> QueryAction {
        let mut action = QueryAction::default();
        ui.horizontal(|ui| {
            ui.label("campaign");
            ui.add_sized([320.0, 22.0], TextEdit::singleline(self.campaign_input));
            if ui.button("Use Selected").clicked()
                && let Some(campaign) = self.selected_campaign
            {
                *self.campaign_input = campaign.to_string();
            }
            if ui
                .add_enabled(
                    self.client_available && !self.query_pending,
                    egui::Button::new("Run raw query"),
                )
                .clicked()
            {
                action.run_raw = true;
            }
            if self.query_pending {
                ui.label(RichText::new("query...").color(Color32::YELLOW));
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Named evidence");
            for (label, view) in [
                ("Relations", WalkEvidenceQuery::Relations),
                ("Counts", WalkEvidenceQuery::Counts),
                ("Config", WalkEvidenceQuery::ConfigEvidence),
                ("Lineage", WalkEvidenceQuery::Lineage),
                ("Progress", WalkEvidenceQuery::Progress),
                ("Handoff", WalkEvidenceQuery::HandoffEvidence),
            ] {
                if ui
                    .add_enabled(
                        self.client_available && !self.query_pending,
                        egui::Button::new(label),
                    )
                    .clicked()
                {
                    action.view = Some(view);
                }
            }
        });
        ui.label(
            "Named views are immutable, revision-tagged evidence projections. Live controller authority remains in Walk status.",
        );
        if !self.client_available {
            ui.label(
                RichText::new(
                    "Select a run with an exact local checkout binding to query its walk service.",
                )
                .color(Color32::YELLOW),
            );
        }
        ui.add_space(8.0);
        ui.heading("Expert raw script");
        if ui.button("Reset to ::relations").clicked() {
            *self.query_script = DEFAULT_QUERY.to_string();
        }
        ui.add_sized(
            [ui.available_width(), 150.0],
            TextEdit::multiline(self.query_script)
                .font(TextStyle::Monospace)
                .desired_rows(7),
        );
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);
        match self.query_result.as_ref() {
            Some(query) => {
                ui.add(QueryResultTable {
                    query,
                    selected_row: self.selected_row,
                });
            }
            None => {
                ui.label(RichText::new("No query result").color(Color32::GRAY));
            }
        }
        action
    }
}

struct QueryResultTable<'a> {
    query: &'a WalkQuerySnapshot,
    selected_row: &'a mut Option<usize>,
}

impl Widget for QueryResultTable<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let result: &DbQueryResult = &self.query.result;
        ui.vertical(|ui| {
            ui.label(format!("campaign: {}", result.campaign_id));
            if let Some(view) = result.view {
                ui.label(format!("named view: {}", view.as_str()));
                egui::CollapsingHeader::new("Executed immutable CozoScript")
                    .id_salt("named_query_script")
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&result.script).monospace()).wrap(),
                        );
                    });
            } else {
                ui.label("executed query:");
                ui.add(egui::Label::new(RichText::new(&result.script).monospace()).wrap());
            }
            ui.add_space(6.0);
            ui.label(RichText::new("Post-snapshot server observation").strong())
                .on_hover_text(
                    "The session revision and phase were observed by the server after the immutable database snapshot; they are not atomic database evidence.",
                );
            ui.horizontal(|ui| {
                ui.label(format!("rows: {}", result.row_count));
                ui.separator();
                ui.label(format!(
                    "revision: {}",
                    short_revision(result.revision.as_str())
                ))
                .on_hover_text(
                    "Content identity of the exact owner database snapshot queried by the walk service",
                );
                ui.separator();
                ui.label(format!(
                    "session revision: {}",
                    self.query.version.journal_revision()
                ))
                .on_hover_text(
                    "Durable controller journal revision observed after the database snapshot",
                );
                ui.separator();
                ui.label(format!("phase: {}", self.query.phase))
                    .on_hover_text("Controller phase observed after the database snapshot");
                ui.separator();
                ui.label(format!("protocol: {}", self.query.epoch.protocol_version));
                ui.separator();
                ui.label(result.db_path.display().to_string());
            });
            ui.add_space(6.0);
            if result.headers.is_empty() {
                ui.label("No headers");
                return;
            }
            ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    Grid::new("query_result_grid")
                        .striped(true)
                        .min_col_width(120.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new("#").strong());
                            for header in &result.headers {
                                ui.label(RichText::new(header).strong());
                            }
                            ui.end_row();
                            for (index, row) in
                                result.rows.iter().take(MAX_TABLE_ROWS).enumerate()
                            {
                                let selected = *self.selected_row == Some(index);
                                if ui.selectable_label(selected, index.to_string()).clicked() {
                                    *self.selected_row = Some(index);
                                }
                                for cell in &row.cells {
                                    ui.label(format_cell(cell));
                                }
                                ui.end_row();
                            }
                        });
                });
        })
        .response
    }
}

fn format_cell(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn short_revision(revision: &str) -> &str {
    revision.get(..12).unwrap_or(revision)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_evidence_views_are_operator_buttons() {
        use egui_kittest::{Harness, kittest::Queryable};

        let mut campaign = "campaign-a".to_string();
        let mut script = DEFAULT_QUERY.to_string();
        let mut selected = None;
        let harness = Harness::builder().build_ui(|ui| {
            QueryPanel {
                campaign_input: &mut campaign,
                query_script: &mut script,
                query_pending: false,
                query_result: None,
                selected_row: &mut selected,
                selected_campaign: None,
                client_available: true,
            }
            .show(ui);
        });

        for label in [
            "Relations",
            "Counts",
            "Config",
            "Lineage",
            "Progress",
            "Handoff",
        ] {
            harness.get_by_label(label);
        }
        harness.get_by_label("Run raw query");
        harness.get_by_label("Expert raw script");
    }

    #[test]
    fn query_actions_are_disabled_without_a_bound_client() {
        use egui_kittest::{
            Harness,
            kittest::{NodeT, Queryable},
        };

        let mut campaign = "campaign-a".to_string();
        let mut script = DEFAULT_QUERY.to_string();
        let mut selected = None;
        let harness = Harness::builder().build_ui(|ui| {
            QueryPanel {
                campaign_input: &mut campaign,
                query_script: &mut script,
                query_pending: false,
                query_result: None,
                selected_row: &mut selected,
                selected_campaign: None,
                client_available: false,
            }
            .show(ui);
        });

        for label in [
            "Run raw query",
            "Relations",
            "Counts",
            "Config",
            "Lineage",
            "Progress",
            "Handoff",
        ] {
            assert!(
                harness.get_by_label(label).accesskit_node().is_disabled(),
                "{label} must be disabled without an exact client binding"
            );
        }
        harness.get_by_label(
            "Select a run with an exact local checkout binding to query its walk service.",
        );
    }

    #[test]
    fn result_table_shows_exact_query_provenance() {
        use egui_kittest::{Harness, kittest::Queryable};

        let query: WalkQuerySnapshot = serde_json::from_value(serde_json::json!({
            "phase": "r4c",
            "result": {
                "repo_root": "/tmp/ploke-parent",
                "campaign_id": "provenance-campaign",
                "db_path": "/tmp/owner.cozo.sqlite",
                "script": "?[name] := *eval_campaign{name}",
                "revision": "abcdef0123456789",
                "headers": ["name"],
                "row_count": 0,
                "rows": []
            },
            "version": {
                "session_id": null,
                "cursor": null,
                "journal_revision": 0
            },
            "epoch": {
                "protocol_version": 9,
                "transition_graph_version": "walk-r0-r14a-v2",
                "repo_root": "/tmp/ploke-parent",
                "exe_path": "/tmp/ploke-eval",
                "exe_modified_unix_ms": 17,
                "git_head": "abc123",
                "active_branch": "parent/runtime-1",
                "source_status_hash": "def456"
            }
        }))
        .expect("typed query result");
        let mut selected = None;
        let harness = Harness::builder()
            .with_size(egui::Vec2::new(900.0, 360.0))
            .build_ui(move |ui| {
                ui.add(QueryResultTable {
                    query: &query,
                    selected_row: &mut selected,
                });
            });

        assert_eq!(
            harness
                .get_all_by_label("campaign: provenance-campaign")
                .count(),
            1
        );
        assert_eq!(
            harness
                .get_all_by_label("?[name] := *eval_campaign{name}")
                .count(),
            1
        );
        assert_eq!(
            harness
                .get_all_by_label("Post-snapshot server observation")
                .count(),
            1
        );
        assert_eq!(harness.get_all_by_label("session revision: 0").count(), 1);
    }
}
