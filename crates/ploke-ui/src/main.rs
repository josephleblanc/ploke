mod app;
mod channels;
mod core;
mod error;
mod state;
mod ui;

// -- external --
use cozo::MemStorage;
use eframe::egui;
use egui::ahash::{HashMap, HashMapExt};
use egui_extras::Column;
use ploke_error::Error;
// -- workspace local imports --
use ploke_db::{Database, QueryResult};
use ploke_transform::schema::{create_schema_all, primary_nodes::FunctionNodeSchema};
use ploke_transform::transform::transform_code_graph;
use serde::{Deserialize, Serialize};
use syn_parser::utils::LogStyle;
use syn_parser::{ParsedCodeGraph, run_phases_and_collect};
// -- std --
#[cfg(feature = "multithreaded")]
use flume::{Sender, bounded};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
#[cfg(feature = "multithreaded")]
use std::thread;
use ui::Anchor;
use ui::query_panel::{QueryBuilderApp, QueryCustomApp};

pub(crate) const LOG_QUERY: &str = "log_query";

struct PlokeApp {
    db: Arc<Database>,
    query: Rc<RefCell<String>>,
    results: Rc<RefCell<Option<Result<QueryResult, Error>>>>,
    last_query_time: Option<std::time::Duration>,
    target_directory: String,
    is_processing: bool,
    processing_status: ProcessingStatus,
    last_processing_time: Option<std::time::Duration>,
    // Channel for receiving status updates
    #[cfg(feature = "multithreaded")]
    status_rx: flume::Receiver<ProcessingStatus>,
    #[cfg(feature = "multithreaded")]
    status_tx: Sender<ProcessingStatus>,

    // Table interaction state
    cells: Rc<RefCell<TableCells>>,
    // TODO: Maybe move the query area into its own app? Trying to follow organization of
    // egui demo here, but might be overkill?
    selected_anchor: ui::Anchor,
    app_query_custom: QueryCustomApp,
    app_query_builder: QueryBuilderApp,
}

#[derive(Clone, PartialEq, PartialOrd, Eq, Ord)]
pub(crate) enum ProcessingStatus {
    Ready,
    #[cfg(feature = "multithreaded")]
    Processing(String),
    #[cfg(not(feature = "multithreaded"))]
    Processing(&'static str),
    Complete,
    #[cfg(feature = "multithreaded")]
    Error(String),
    Error(String),
}

impl std::fmt::Display for ProcessingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessingStatus::Ready => write!(f, "Ready"),
            ProcessingStatus::Processing(msg) => write!(f, "Processing: {}", msg),
            ProcessingStatus::Complete => write!(f, "Complete"),
            ProcessingStatus::Error(err) => write!(f, "Error: {}", err),
        }
    }
}

/// Contains data for the UI to reference the selected cells in the returned value from a query.
pub struct TableCells {
    selected_cells: Vec<(usize, usize)>, // (row, column) indices
    selection_in_progress: Option<(usize, usize)>, // Starting cell for drag selection
    control_groups: HashMap<usize, Vec<(usize, usize)>>, // Control group number -> cells
}

impl TableCells {
    pub(crate) fn reset_selected_cells(cells: &mut TableCells) {
        cells.selected_cells.clear();
        cells.selection_in_progress = None;
        cells.control_groups.clear();
    }
}

impl Default for TableCells {
    fn default() -> Self {
        Self {
            selected_cells: Vec::new(),
            selection_in_progress: None,
            control_groups: HashMap::new(),
        }
    }
}

impl PlokeApp {
    fn new() -> Self {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .try_init();

        let db = cozo::Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        let db = Arc::new(Database::new(db));

        let cells = Rc::new(RefCell::new(TableCells::default()));
        let query_results = Rc::new(RefCell::new(None));

        let default_query = Rc::new(RefCell::new(String::from(
            "?[name, id, body] := *function { name, id, body }",
        )));

        // Initialize schemas
        create_schema_all(&db).expect("Failed to create schemas");

        // Create channel with backpressure (100 message buffer)
        #[cfg(feature = "multithreaded")]
        let (status_tx, status_rx) = bounded(100);

        Self {
            db: Arc::clone(&db),
            query: default_query,
            results: Rc::clone(&query_results),
            last_query_time: None,
            target_directory: String::from(
                "/home/brasides/code/second_aider_dir/ploke/tests/fixture_crates/fixture_nodes",
            ),
            is_processing: false,
            processing_status: ProcessingStatus::Ready,
            last_processing_time: None,
            #[cfg(feature = "multithreaded")]
            status_rx,
            #[cfg(feature = "multithreaded")]
            status_tx,
            cells: Rc::clone(&cells),
            selected_anchor: ui::Anchor::QueryCustom,
            app_query_custom: QueryCustomApp {
                custom_query: String::from("?[name, id, body] := *function { name, id, body }"),
                db,
                cells,
                results: query_results,
            },
            app_query_builder: QueryBuilderApp {
                current_builder_query: String::from("?[name, id] := *function { name, id }"),
                db: Arc::clone(&db),
                cells: Rc::clone(&cells),
            },
        }
    }

    fn query_bar_contents(
        &mut self,
        ui: &mut egui::Ui,
        frame: &mut eframe::Frame,
        cmd: &mut Command,
    ) {
        let mut selected_anchor = self.selected_anchor;
        // Iterates over the buttons, both drawing them and setting the selected anchor based on
        // the clicked button.
        for (name, anchor, _app) in self.apps_iter_mut() {
            if ui
                .selectable_label(selected_anchor == anchor, name)
                .clicked()
            {
                selected_anchor = anchor;
            }
        }

        // original on egui demo is self.state.selected_anchor
        self.selected_anchor = selected_anchor;

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {});
        todo!()
    }

    pub fn apps_iter_mut(
        &mut self,
    ) -> impl Iterator<Item = (&'static str, Anchor, &mut dyn eframe::App)> {
        vec![
            (
                "Custom Query",
                Anchor::QueryCustom,
                &mut self.app_query_custom as &mut dyn eframe::App,
            ),
            (
                "Query Builder",
                Anchor::QueryBuilder,
                &mut self.app_query_builder as &mut dyn eframe::App,
            ),
        ]
        .into_iter()
    }
}

#[derive(Clone, Copy, Debug)]
#[must_use]
enum Command {
    Nothing,
    ResetEverything,
}

impl eframe::App for PlokeApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Check for async status updates
        #[cfg(feature = "multithreaded")]
        if let Ok(status) = self.status_rx.try_recv() {
            self.processing_status = status;
        }
        if self.processing_status == ProcessingStatus::Complete {
            self.is_processing = false;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Ploke Codegraph Processor");

            // Target directory section
            ui.horizontal(|ui| {
                ui.label("Target Directory:");
                ui.text_edit_singleline(&mut self.target_directory);
                if ui.button("Browse...").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.target_directory = path.display().to_string();
                    }
                }
            });

            // Process button
            ui.horizontal(|ui| {
                ui.add_enabled_ui(
                    !self.is_processing && !self.target_directory.is_empty(),
                    |ui| {
                        if ui.button("Process Target").clicked() {
                            self.process_target();
                        }
                    },
                );
            });
            ui.horizontal(|ui| {
                ui.label(self.processing_status.to_string());
                if let Some(duration) = self.last_processing_time {
                    ui.label(format!("(Last run: {:.2?})", duration));
                }
            });

            // Query section
            ui.separator();
            ui.heading("Query Database");
            let mut cmd = Command::Nothing;
            egui::TopBottomPanel::top("query_app_top_bar")
                .frame(egui::Frame::new().inner_margin(4))
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.visuals_mut().button_frame = false;
                        self.query_bar_contents(ui, frame, &mut cmd);
                    })
                });
            ui.horizontal(|ui| {
                // Moved logic for handling items that might have different tabs to the query
                // apps in `query_panel.rs`
            });

            if ui.button("Execute").clicked() {
                self.reset_selected_cells();
                self.execute_query();
            }
            // ui.collapsing("Schema", |ui| {
            //     ui.collapsing("function", |ui| {
            //     })
            // })

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Results:");
                if let Some(duration) = self.last_query_time {
                    ui.label(format!("(Query took: {:.2?})", duration));
                }
            });

            let query_result = self.results.borrow();
            if let Some(q_header_rows) = &*query_result {
                match q_header_rows {
                    Ok(q) => {
                        let q = q.clone(); // Clone the result to avoid borrow issues
                        drop(query_result);
                        self.render_cozo_table(ui, &q);
                    }
                    Err(e) => {
                        ui.label(format!("{:#?}", e));
                    }
                }
            } else {
                ui.label("No results to show yet.");
            }
        });
    }
}

impl PlokeApp {
    #[cfg(feature = "multithreaded")]
    fn process_target_parallel(&mut self) {
        self.is_processing = true;
        self.processing_status = ProcessingStatus::Processing("Starting up...".to_string());

        let target_dir = self.target_directory.clone();
        let db = Arc::clone(&self.db);
        let status_tx = self.status_tx.clone();

        thread::spawn(move || -> Result<ProcessingStatus, Error> {
            status_tx
                .send(ProcessingStatus::Processing("Initializing...".to_string()))
                .map_err(UiError::from)?;

            let successful_graphs = run_phases_and_collect(&target_dir)?;

            status_tx
                .send(ProcessingStatus::Processing(
                    "Merging graphs...".to_string(),
                ))
                .map_err(UiError::from)?;
            let merged = ParsedCodeGraph::merge_new(successful_graphs)?;

            status_tx
                .send(ProcessingStatus::Processing(
                    "Creating module tree...".to_string(),
                ))
                .map_err(UiError::from)?;
            let tree = merged.build_module_tree()?;

            // Create schemas and transform data
            // TODO: Change transform_code_graph to take `ParsedCodeGraph` instead, once we have
            // added a transform of the crate info.
            status_tx
                .send(ProcessingStatus::Processing(
                    "Transforming data...".to_string(),
                ))
                .map_err(UiError::from)?;
            if let Err(e) = transform_code_graph(&db, merged.graph, &tree) {
                status_tx
                    .send(ProcessingStatus::Error(format!("Transform error: {}", e)))
                    .unwrap();
            }

            status_tx
                .send(ProcessingStatus::Complete)
                .map_err(UiError::from)?;
            Ok(ProcessingStatus::Complete)
        });
    }
    fn process_target(&mut self) {
        let start_time = std::time::Instant::now();
        self.is_processing = true;
        self.processing_status = ProcessingStatus::Processing("Starting up...");

        let target_dir = self.target_directory.clone();
        #[cfg(feature = "multithreaded")]
        let db = Arc::clone(&self.db);
        #[cfg(feature = "multithreaded")]
        let status_tx = self.status_tx.clone();

        // Use a single-threaded approach with immediate processing
        if let Err(e) = self.do_processing(target_dir) {
            self.processing_status = ProcessingStatus::Error(e.to_string());
            self.is_processing = false;
        }
        self.last_processing_time = Some(start_time.elapsed());
        #[cfg(feature = "multithreaded")]
        match self.do_processing(target_dir, db, status_tx) {
            Ok(duration) => {
                self.last_processing_time = Some(duration);
                self.is_processing = false;
            }
            Err(e) => {
                self.processing_status = ProcessingStatus::Error(e.to_string());
                self.is_processing = false;
            }
        }
    }

    fn do_processing(
        &mut self,
        target_dir: String,
        #[cfg(feature = "multithreaded")] db: Arc<Database>,
        #[cfg(feature = "multithreaded")] status_tx: Sender<ProcessingStatus>,
    ) -> Result<(), Error> {
        let successful_graphs = run_phases_and_collect(&target_dir)?;

        #[cfg(feature = "multithreaded")]
        status_tx
            .send(ProcessingStatus::Processing(
                "Merging graphs...".to_string(),
            ))
            .map_err(UiError::from)?;
        self.processing_status = ProcessingStatus::Processing("Merging Graphs...");
        let merged = ParsedCodeGraph::merge_new(successful_graphs)?;

        #[cfg(feature = "multithreaded")]
        status_tx
            .send(ProcessingStatus::Processing(
                "Creating module tree...".to_string(),
            ))
            .map_err(UiError::from)?;
        self.processing_status = ProcessingStatus::Processing("Creating module tree...");
        let tree = merged.build_module_tree()?;

        #[cfg(feature = "multithreaded")]
        status_tx
            .send(ProcessingStatus::Processing(
                "Transforming data...".to_string(),
            ))
            .map_err(UiError::from)?;
        self.processing_status = ProcessingStatus::Processing("Transforming data...");
        transform_code_graph(&self.db, merged.graph, &tree)?;

        #[cfg(feature = "multithreaded")]
        status_tx
            .send(ProcessingStatus::Complete)
            .map_err(UiError::from)?;
        self.processing_status = ProcessingStatus::Complete;
        Ok(())
    }

    fn execute_query(&mut self) {
        let start_time = std::time::Instant::now();
        let query = match self.selected_anchor {
            Anchor::QueryCustom => self.app_query_custom.custom_query.clone(),
            Anchor::QueryBuilder => self.app_query_builder.current_builder_query.clone(),
        };
        let db_results = Some(self.db.raw_query(&query).map_err(Error::from));
        self.results = Rc::new(RefCell::new(db_results));
        self.last_query_time = Some(start_time.elapsed());
        // query logging
        match &*self.results.borrow() {
            Some(Ok(result)) => {
                log::info!(target: LOG_QUERY,
                    "{} {} | Number of matches: {}",
                    "Query Status:".log_step(),
                    "Success".log_spring_green(),
                    result.rows.len()
                );
            }
            Some(Err(e)) => {
                log::error!(target: LOG_QUERY,
                    "{} {} | {:#?}",
                    "Query Status:".log_step(),
                    "Error".log_error(),
                    e
                );
            }
            None => {}
        }
    }

    fn render_cozo_table(&mut self, ui: &mut egui::Ui, q: &QueryResult) {
        let num_rows = q.rows.len();
        let num_cols = q.headers.len();
        // Give the table a unique ID if you have multiple tables in the same UI
        // let table_id = egui::Id::new("cozo_table");

        egui::ScrollArea::both().show(ui, |ui| {
            let table = egui_extras::TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .columns(Column::auto().resizable(true).clip(true), num_cols) // Define number of columns
                // Or, for more control over individual columns:
                // .columns(Column::initial(100.0).resizable(true), num_cols -1)
                // .column(Column::remainder().resizable(false)) // Last column takes remaining space
                .min_scrolled_height(0.0); // Optional: useful for small tables
            table
                .header(20.0, |mut header| {
                    // Define header row
                    for col_name in &q.headers {
                        header.col(|ui| {
                            ui.strong(col_name);
                        });
                    }
                })
                .body(|mut body| {
                    // Define body of the table
                    if num_rows > 0 {
                        body.rows(
                            18.0, // Row height
                            num_rows,
                            |mut row| {
                                let row_index = row.index();
                                if let Some(data_row) = q.rows.get(row_index) {
                                    for (col_index, cell_value) in data_row.iter().enumerate() {
                                        row.col(|ui| {
                                            let is_selected = {
                                                let cells = self.cells.borrow();
                                                cells.selected_cells.contains(&(row_index, col_index))
                                            };

                                            // Create a frame with background color if selected
                                            let frame = if is_selected {
                                                egui::Frame::NONE
                                                    .fill(egui::Color32::from_rgb(70, 130, 180))
                                                    .inner_margin(egui::Margin::same(2))
                                            } else {
                                                egui::Frame::none()
                                                    .inner_margin(egui::Margin::same(2))
                                            };

                                            // Render the cell with the frame
                                            frame.show(ui, |ui| {
                                                    // Use selectable label for better interaction
                                                    let response = ui.selectable_label(
                                                        is_selected,
                                                        cell_value.to_string(),
                                                    );

                                                    // Handle click to select/deselect
                                                    // Handle click to select/deselect
                                                    if response.clicked() {
                                                        let cell = (row_index, col_index);
                                                        if let Ok(mut cells) = self.cells.try_borrow_mut() {
                                                            if cells.selected_cells.contains(&cell) {
                                                                cells.selected_cells.retain(|&c| c != cell);
                                                            } else {
                                                                cells.selected_cells.push(cell);
                                                            }
                                                        }
                                                    }

                                                    // Handle drag start
                                                    if response.drag_started() {
                                                        if let Ok(mut cells) = self.cells.try_borrow_mut() {
                                                            cells.selection_in_progress = Some((row_index, col_index));
                                                        }
                                                    }

                                                    // Handle ongoing drag
                                                    if response.dragged() {
                                                        if let Ok(mut cells) = self.cells.try_borrow_mut() {
                                                        cells.selection_in_progress = None;
                                                    }
                                                    }
                                                });
                                        });
                                    }
                                }
                            },
                        );
                    } else {
                        // Handle case with headers but no data rows
                        body.row(18.0, |mut row| {
                            row.col(|ui| {
                                ui.label("No data.");
                            });
                            // Add empty cells for other columns if desired
                            for _ in 1..num_cols {
                                row.col(|_ui| {});
                            }
                        });
                    }
                });
        });
    }
    fn handle_cell_click(&mut self, row: usize, col: usize) {
        let cell = (row, col);

        // Toggle selection state
        self.modfy_table_cell(cell);
    }

    fn modfy_table_cell(&mut self, cell: (usize, usize)) {
        if let Ok(mut cells) = self.cells.try_borrow_mut() {
            fun_name(cell, &mut cells);
        }
    }

    fn update_drag_selection(&mut self, current_row: usize, current_col: usize) {
        // Clear previous selection
        match self.cells.try_borrow_mut() {
            Ok(mut cells) => {

            if let Some((start_row, start_col)) = cells.selection_in_progress {
                cells.selected_cells.clear();
                    // Calculate the rectangle of selected cells
                    let min_row = start_row.min(current_row);
                    let max_row = start_row.max(current_row);
                    let min_col = start_col.min(current_col);
                    let max_col = start_col.max(current_col);

                    // Add all cells in the rectangle to selection
                    for row in min_row..=max_row {
                        for col in min_col..=max_col {
                            cells.selected_cells.push((row, col));
                        }
                    }
                }
            }
            Err(e) => log_cell_error(e),
        }
    }

    pub(crate) fn reset_selected_cells(&mut self) {
        if let Ok(mut cells) = self.cells.try_borrow_mut() {
            cells.selected_cells.clear();
            cells.selection_in_progress = None;
            cells.control_groups.clear();
        }
    }
}

fn fun_name(cell: (usize, usize), cells: &mut TableCells) {
    if cells.selected_cells.contains(&cell) {
        cells.selected_cells.retain(|&c| c != cell);
    } else {
        cells.selected_cells.push(cell);
    }
}

fn log_cell_error(e: std::cell::BorrowMutError) {
    log::error!(target: LOG_QUERY,
        "{} {} | {}",
        "borrow_mut Error".log_error(),
        "reset_selected_cells".log_step(),
        e.to_string()
    );
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Ploke UI",
        options,
        Box::new(|_cc| Ok(Box::new(PlokeApp::new()))),
    )
}
