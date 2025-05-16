use cozo::MemStorage;
use eframe::egui;
use std::sync::mpsc;
use std::thread;

struct PlokeApp {
    db: cozo::Db<MemStorage>,
    query: String,
    results: String,
    target_directory: String,
    is_processing: bool,
    processing_status: String,
    // Channel for receiving status updates
    status_receiver: Option<mpsc::Receiver<String>>,
}

impl Default for PlokeApp {
    fn default() -> Self {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .try_init();

        let db = cozo::Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");

        Self {
            db,
            query: String::new(),
            results: String::new(),
            target_directory: String::new(),
            is_processing: false,
            processing_status: String::from("Ready"),
            status_receiver: None,
        }
    }
}

impl eframe::App for PlokeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for status updates from processing thread
        if let Some(receiver) = &self.status_receiver {
            if let Ok(status) = receiver.try_recv() {
                self.processing_status = status;
                if status.contains("complete") || status.contains("error") {
                    self.is_processing = false;
                }
            }
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
                ui.set_enabled(!self.is_processing && !self.target_directory.is_empty());
                if ui.button("Process Target").clicked() {
                    self.process_target();
                }
                ui.label(&self.processing_status);
            });

            // Query section
            ui.separator();
            ui.heading("Query Database");
            ui.horizontal(|ui| {
                ui.label("Query:");
                ui.text_edit_multiline(&mut self.query);
            });

            if ui.button("Execute").clicked() {
                self.execute_query();
            }

            ui.separator();
            ui.label("Results:");
            ui.text_edit_multiline(&mut self.results).lock_focus(true);
        });
    }
}

impl PlokeApp {
    fn process_target(&mut self) {
        self.is_processing = true;
        self.processing_status = String::from("Processing...");

        let target_dir = self.target_directory.clone();
        let db = self.db.clone();

        // Create channel for status updates
        let (sender, receiver) = mpsc::channel();
        self.status_receiver = Some(receiver);

        thread::spawn(move || {
            sender.send("Initializing...".to_string()).ok();

            // Run the parser phases
            let successful_graphs = match run_phases_and_collect(&target_dir) {
                Ok(graphs) => graphs,
                Err(e) => {
                    sender.send(format!("Error: {}", e)).ok();
                    return;
                }
            };

            sender.send("Merging graphs...".to_string()).ok();
            let merged = match ParsedCodeGraph::merge_new(successful_graphs) {
                Ok(m) => m,
                Err(e) => {
                    sender.send(format!("Merge error: {}", e)).ok();
                    return;
                }
            };

            // Create schemas and transform data
            sender.send("Creating schemas...".to_string()).ok();
            if let Err(e) = ConstNodeSchema::SCHEMA.create_and_insert(&db) {
                sender.send(format!("Schema error: {}", e)).ok();
                return;
            }

            if let Err(e) = AttributeNodeSchema::SCHEMA.create_and_insert(&db) {
                sender.send(format!("Schema error: {}", e)).ok();
                return;
            }

            sender.send("Transforming data...".to_string()).ok();
            if let Err(e) = transform_consts(&db, merged.graph.consts) {
                sender.send(format!("Transform error: {}", e)).ok();
                return;
            }

            sender.send("Processing complete!".to_string()).ok();
        });
    }

    fn execute_query(&mut self) {
        match self.db.run_script(&self.query, Default::default()) {
            Ok(result) => {
                self.results = format!("{:#?}", result);
            }
            Err(e) => {
                self.results = format!("Query error: {}", e);
            }
        }
    }
}

pub fn run() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Ploke UI",
        options,
        Box::new(|_cc| Box::<PlokeApp>::default()),
    )
}
