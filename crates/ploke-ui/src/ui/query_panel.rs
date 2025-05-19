use std::{cell::RefCell, rc::Rc, sync::Arc};

use ploke_db::{Database, QueryResult};
use ploke_error::Error;
use serde::{Deserialize, Serialize};
use syn_parser::utils::LogStyle;

use crate::{TableCells, LOG_QUERY};

pub struct QueryCustomApp {
    // Custom Query specific state
    pub custom_query: String,
    // Shared state references
    pub db: Arc<Database>,
    pub cells: Rc<RefCell<TableCells>>,
    pub results: Rc<RefCell<Option<Result<QueryResult, Error>>>>,
}

impl<'a> eframe::App for QueryCustomApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // NOTE: cells is mut for now, but could be immut here.
        if let Ok( cells ) = self.cells.try_borrow_mut() {
            let results = self.results.borrow();

            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label("Query:");
                ui.code_editor(&mut self.custom_query);

        if let Some(Ok(q)) = &*results {
                    let iter_selected = cells
                    .selected_cells
                    .iter()
                    .map(|(i, j)| format!("{}", q.rows[*i][*j]));

                ui.vertical(|ui| {
                    ui.label("Selected Items:");
                    // ui.horizontal_top(|ui| {
                            for item in iter_selected {
                                if ui.button("Add Filter").clicked() {
                                    println!("Do something!!!");
                                }
                                ui.label(item);
                            }
                    // })
                });
            } else {
                log::warn!(target: LOG_QUERY, 
                    "{} {} | {:#?}",
                    "QueryCustomApp".log_header(),
                    "Database Error".log_error(),
                    "e"
                );

                }
            });
            } else {
                log::warn!(target: LOG_QUERY, 
                "{} {} {}",
                "QueryCustomApp".log_header(),
                "Accessing TableCells".log_step(),
                "Blocked access to mutable state self.cells".log_foreground_primary()
            );
            }
    }
}

pub struct QueryBuilderApp {
    // Builder-specific state
    pub current_builder_query: String,
    // Shared state references
    pub db: Arc<Database>,
    pub cells: Rc<RefCell<TableCells>>,
}

impl eframe::App for QueryBuilderApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Query Builder");

            ui.horizontal(|ui| {
                if ui.button("Function").clicked() {
                    self.current_builder_query.push_str("?[name, id] := *function { name, id }");
                }
                if ui.button("Struct").clicked() {
                    self.current_builder_query.push_str("?[name, id] := *struct { name, id }");
                }
            });

            // Query preview
            ui.separator();
            ui.label("Query Preview:");
            // NOTE: I don't think code_editor really needs to be &mut
            ui.code_editor(&mut self.current_builder_query);

            // Selected items from table (shared state)
            let cells = self.cells.borrow();
            ui.separator();
            ui.label("Selected Items");

            if cells.selected_cells.is_empty() {
                ui.label("No selections");
            } else {
                // NOTE: Maybe use vertical_wrapped if that exists?
                ui.horizontal_wrapped(|ui| {
                    for (row, col) in &cells.selected_cells {
                        ui.label(format!("Cell ({}, {})", row, col));
                    }
                });
            }

            // Query builder controls
            if ui.button("Add Filter").clicked() {
                // Example: Add filter for selected items
                if !cells.selected_cells.is_empty() {
                    self.current_builder_query.push_str("\nfilter id = $selected_id");
                }
            }
        });
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[must_use]
pub enum Anchor {
    QueryCustom,
    QueryBuilder,
}

impl Anchor {
    fn all() -> Vec<Self> {
        vec![Self::QueryCustom, Self::QueryBuilder]
    }

    fn from_str_case_insensitive(anchor: &str) -> Option<Self> {
        let anchor = anchor.to_lowercase();
        Self::all().into_iter().find(|x| x.to_string() == anchor)
    }
}

impl std::fmt::Display for Anchor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut name = format!("{self:?}");
        name.make_ascii_lowercase();
        f.write_str(&name)
    }
}

impl From<Anchor> for egui::WidgetText {
    fn from(value: Anchor) -> Self {
        Self::from(value.to_string())
    }
}

impl Default for Anchor {
    fn default() -> Self {
        Self::QueryCustom
    }
}
