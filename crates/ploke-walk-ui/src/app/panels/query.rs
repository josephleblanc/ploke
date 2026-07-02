use eframe::egui::{
    self, Color32, Grid, Response, RichText, ScrollArea, TextEdit, TextStyle, Ui, Widget,
};
use ploke_eval::walk_client::DbQueryResult;

use crate::model::{DEFAULT_QUERY, MAX_TABLE_ROWS};

pub(in crate::app) struct QueryPanel<'a> {
    pub(in crate::app) campaign_input: &'a mut String,
    pub(in crate::app) query_script: &'a mut String,
    pub(in crate::app) query_pending: bool,
    pub(in crate::app) query_result: Option<&'a DbQueryResult>,
    pub(in crate::app) selected_row: &'a mut Option<usize>,
    pub(in crate::app) selected_campaign: Option<&'a str>,
}

#[derive(Debug, Default)]
pub(in crate::app) struct QueryAction {
    pub(in crate::app) run_query: bool,
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
                .add_enabled(!self.query_pending, egui::Button::new("Run Query"))
                .clicked()
            {
                action.run_query = true;
            }
            if ui.button("Relations").clicked() {
                *self.query_script = DEFAULT_QUERY.to_string();
            }
            if self.query_pending {
                ui.label(RichText::new("query...").color(Color32::YELLOW));
            }
        });
        ui.add_space(8.0);
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
            Some(result) => {
                ui.add(QueryResultTable {
                    result,
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
    result: &'a DbQueryResult,
    selected_row: &'a mut Option<usize>,
}

impl Widget for QueryResultTable<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(format!("rows: {}", self.result.row_count));
                ui.separator();
                ui.label(self.result.db_path.display().to_string());
            });
            ui.add_space(6.0);
            if self.result.headers.is_empty() {
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
                            for header in &self.result.headers {
                                ui.label(RichText::new(header).strong());
                            }
                            ui.end_row();
                            for (index, row) in
                                self.result.rows.iter().take(MAX_TABLE_ROWS).enumerate()
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
