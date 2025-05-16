use cozo::MemStorage;
use eframe::egui;

struct PlokeApp {
    // TODO:
    // db: ploke_db::Database,
    db: cozo::Db<MemStorage>,
    query: String,
    results: String,
}

impl Default for PlokeApp {
    fn default() -> Self {
        let db = cozo::Db::new(MemStorage::default()).expect("Failed to create database");
        db.initialize().expect("Failed to initialize database");
        Self {
            // TODO:
            // db: ploke_db::Database::new(),
            db,
            query: String::new(),
            results: String::new(),
        }
    }
}

impl eframe::App for PlokeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Ploke Codegraph Query");

            // Query input
            ui.horizontal(|ui| {
                ui.label("Query:");
                ui.text_edit_multiline(&mut self.query);
            });

            // Execute button
            if ui.button("Execute").clicked() {
                self.execute_query();
            }

            // Results display
            ui.separator();
            ui.label("Results:");
            ui.text_edit_multiline(&mut self.results);
        });
    }
}

impl PlokeApp {
    fn execute_query(&mut self) {
        // TODO: Implement query execution using cozo
        self.results = "Query results will appear here".to_string();
    }
}

pub fn run() -> std::result::Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Ploke UI",
        options,
        Box::new(|_cc| Box::<PlokeApp>::default()),
    )
    // TODO: Error Handling
    // .map_err(Into::into)
}
