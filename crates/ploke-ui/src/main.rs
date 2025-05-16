// -- external --
use cozo::MemStorage;
use eframe::egui;
use egui_extras::Column;
use error::UiError;
use ploke_error::Error;
// -- workspace local imports --
use ploke_db::{Database, QueryResult};
use ploke_transform::schema::create_schema_all;
use ploke_transform::transform::transform_code_graph;
use syn_parser::{ParsedCodeGraph, run_phases_and_collect};
// -- std --
#[cfg(feature = "multithreaded")]
use flume::{Sender, bounded};
use std::sync::Arc;
#[cfg(feature = "multithreaded")]
use std::thread;

mod error;

struct PlokeApp {
    db: Arc<Database>,
    query: String,
    results: Option<Result<QueryResult, Error>>,
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
}

#[derive(Clone)]
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

impl PlokeApp {
    fn new() -> Self {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .try_init();

        let db = cozo::Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        let db = Arc::new(Database::new(db));

        // Initialize schemas
        create_schema_all(&db).expect("Failed to create schemas");

        // Create channel with backpressure (100 message buffer)
        #[cfg(feature = "multithreaded")]
        let (status_tx, status_rx) = bounded(100);

        Self {
            db,
            query: String::from("?[name, id, body] := *function { name, id, body }"),
            results: None,
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
        }
    }
}

impl eframe::App for PlokeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for async status updates
        #[cfg(feature = "multithreaded")]
        if let Ok(status) = self.status_rx.try_recv() {
            self.processing_status = status;
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
            ui.horizontal(|ui| {
                ui.label("Query:");
                ui.code_editor(&mut self.query);
            });

            if ui.button("Execute").clicked() {
                self.execute_query();
            }

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Results:");
                if let Some(duration) = self.last_query_time {
                    ui.label(format!("(Query took: {:.2?})", duration));
                }
            });

            // Try to parse as JSON first (common Cozo output format)

            if let Some(query_result) = &self.results {
                match query_result {
                    Ok(q_header_rows) => {
                        self.render_cozo_table(ui, q_header_rows);
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
        self.results = Some(self.db.raw_query(&self.query).map_err(Error::from));
        self.last_query_time = Some(start_time.elapsed());
        // match self.db.raw_query(&self.query) {
        //     Ok(result) => {
        //         self.results = Some(result);
        //     }
        //     Err(e) => {
        //         self.results = {
        //             format!("Query error: {}", e)
        //         };
        //     }
        // }
    }

    fn render_cozo_table(&self, ui: &mut egui::Ui, q: &QueryResult) {
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
                                    for cell_value in data_row {
                                        row.col(|ui| {
                                            // Convert DataValue to a string for display
                                            // You might want more sophisticated rendering
                                            // for different DataValue types here.
                                            ui.label(cell_value.to_string());
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
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Ploke UI",
        options,
        Box::new(|_cc| Ok(Box::new(PlokeApp::new()))),
    )
}
