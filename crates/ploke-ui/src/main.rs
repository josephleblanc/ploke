mod app;
mod channels;
mod core;
mod error;
mod state;
mod ui;

// -- external --
use cozo::MemStorage;
use eframe::egui;
use egui::{Button, FontId, Label, RichText, TextEdit, TextStyle};
use egui_extras::Column;
use ploke_db::{Database, FieldValue, NodeType, QueryResult};
use ploke_error::Error;
use ploke_transform::schema::create_schema_all;
use ploke_transform::schema::primary_nodes::FunctionNodeSchema;
use ploke_transform::transform::transform_code_graph;
use syn_parser::utils::{LogStyle, LogStyleDebug};
use syn_parser::{ParsedCodeGraph, run_phases_and_collect};
// -- std --
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
// -- local ui elements --
use ui::query_panel::{QueryBuilderApp, QueryCustomApp};
use ui::{Anchor, TableCells};
// -- conditional --
#[cfg(feature = "multithreaded")]
use flume::{Sender, bounded};
#[cfg(feature = "multithreaded")]
use std::thread;

pub(crate) const LOG_QUERY: &str = "log_query";

struct PlokeApp {
    db: Arc<Database>,
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
    query_section_id: Option<egui::Id>,
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

        // Initialize schemas
        create_schema_all(&db).expect("Failed to create schemas");

        // Create channel with backpressure (100 message buffer)
        #[cfg(feature = "multithreaded")]
        let (status_tx, status_rx) = bounded(100);

        let prepopulated_builder = ploke_db::QueryBuilder::new()
            .functions()
            .add_lhs("name")
            .add_lhs("id");
        let current_builder_query = prepopulated_builder.lhs_to_query_string();

        Self {
            db: Arc::clone(&db),
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
                db: db.clone(),
                cells: cells.clone(),
                results: query_results,
                selected_schema: NodeType::Function.to_base_query(),
            },
            app_query_builder: QueryBuilderApp {
                current_builder_query: format!(
                    "{} := *function {{ name, id }}",
                    current_builder_query
                ),
                db: Arc::clone(&db),
                cells: Rc::clone(&cells),
                query_builder: prepopulated_builder,
                selected_node_type: NodeType::Function,
            },
            query_section_id: None,
        }
    }

    fn query_bar_contents(
        &mut self,
        ui: &mut egui::Ui,
        _frame: &mut eframe::Frame,
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

    fn modify_table_cell<F>(&self, action: F)
    where
        F: FnOnce(&mut TableCells),
    {
        match self.cells.try_borrow_mut() {
            Ok(mut cells) => {
                action(&mut cells);
            }
            Err(e) => {
                log_cell_error(e);
            }
        }
    }

    /// Render the custom query section, which provides an interactable way for the user to build a
    /// query to the cozo database.
    /// The query builder section occupies the same space as the "custom query" section, and only
    /// one of them should be displayed at a time.
    fn render_query_builder(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            // self.render_relation_type_col(ui);
            ui.push_id("query_builder_preview", |ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new("Query Preview:").font(FontId::proportional(16.0)));
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // NOTE: I don't think code_editor really needs to be &mut
                        ui.code_editor(&mut self.app_query_builder.current_builder_query);
                    });
                });
            });
        });
    }

    fn render_query_builder_v2(&mut self, ui: &mut egui::Ui) {
        egui::SidePanel::left("left_hand_side")
            .show_separator_line(true)
            .show_inside(ui, |ui| {
                ui.label(RichText::new("Left-Hand Side:").font(FontId::proportional(14.0)));
                ui.separator();
                ui.horizontal(|ui| {
                    // NOTE: The following was taken from the egui demo but doesn't seem to be
                    // doing what I want here, which is to provide indentation for the items in the
                    // code that would follow a similar indentation to something like:
                    // ?[
                    //      item,
                    //      item2,
                    // ]
                    // let font_id = egui::TextStyle::Monospace.resolve(ui.style());
                    // let indentation = 2.0 * 4.0 * ui.fonts(|f| f.glyph_width(&font_id, ' '));
                    // ui.add_space(indentation);

                    egui::Grid::new("query_builder_interactive")
                        .striped(true)
                        .num_columns(2)
                        .show(ui, |ui| {
                            self.lhs_grid(ui);
                        });
                });
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.label(RichText::new("Right-Hand Side:").font(FontId::proportional(14.0)));
            ui.horizontal(|ui| {
                // let font_id = egui::TextStyle::Monospace.resolve(ui.style());
                // let indentation = 2.0 * 4.0 * ui.fonts(|f| f.glyph_width(&font_id, ' '));
                // ui.add_space(indentation);

                egui::Grid::new("query_builder_interactive")
                    .striped(true)
                    .num_columns(2)
                    .show(ui, |ui| {
                        self.rhs_grid(ui);
                    });
            });
        });
    }

    /// The interactable, dynamic grid with the right-hand side of a cozo query.
    ///
    /// The user should be able to add new terms to the right hand side, and decide whether these
    /// terms are being added with the `and`, `or`, or `not` modifiers, or with the default `,`,
    /// which by default the `,` comma is the same as the `and` combining term, except that it has
    /// lower priority than the `or` or `and` keywords.
    ///
    /// The user first selects which `relation` they want to add to the rhs, then are presented
    /// with a dropdown that allows the user to select a field for inclusion. After the field is
    /// selected for inclusion, a new text box will appear to the right of the selected term, which
    /// allows for optionally choosing a value for that field (which acts as a filter in the
    /// search), or to bind that field to a variable (after which that bound variable may be used
    /// again in following `relation`s added to the rhs).
    ///
    /// The overall flow is to:
    ///     1. Select the node type from the dropdown
    ///     2. Select the field for inclusion
    ///     3. (optionally) specify a value or variable binding for the field
    ///     4. (optionally) add another field using another `+` symbol menu icon that will appear
    ///        in the row following the recently selected field.
    ///     5. (optionally) specifiy an `and`, `or`, or `not` modifier to the relation term. This
    ///        will appear as a dropdown menu at end of each `relation` term, and will be a
    ///        dropdown menu item which by default holds a `,`, but has selectable items for `and`
    ///        and `or`. `not` will be selectable as a checkbox to the left of the row with the
    ///        field.
    fn rhs_grid(&mut self, ui: &mut egui::Ui) {
        let query_builder = &mut self.app_query_builder.query_builder;
        let builder_preview = &mut self.app_query_builder.current_builder_query;

        // ------------------------------
        // ------------------------------
        // show_code(ui, "");
        ui.menu_button("+", |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for relation in NodeType::all_variants() {
                    if ui.button(relation.relation_str()).clicked() {
                        query_builder.insert_rhs_rel(relation);
                    }
                }
            });
        });
        ui.end_row();
        // ------------------------------
        let mut to_remove = None;
        let mut field_to_remove = None;
        for (k, v) in query_builder.rhs_rels.iter_mut() {
            // show title of relation type on one line, e.g. *function or *struct
            ui.horizontal(|ui| {
                if ui.button("-").clicked() {
                    to_remove = Some(*k);
                }
                show_code(ui, k.node_type.relation_str());
            });
            ui.end_row();

            // on the following rows, show the selected fields
            for fv in v.iter_mut() {
                ui.label("\t");
                if ui.button("-").clicked() {
                    field_to_remove = Some((k.clone(), fv.clone()));
                }
                show_code(ui, fv.field);
                show_code(ui, ": ");
                ui.add(
                    TextEdit::singleline(&mut fv.value).clip_text(false), // TODO: Add ghost text for the expected datatype here
                );
                show_code(ui, ",");
                ui.end_row();
            }
            ui.menu_button("+", |ui| {
                for new_field in k.node_type.fields() {
                    if !v.iter().any(|fv| &fv.field == new_field) && ui.button(*new_field).clicked()
                    {
                        v.push(FieldValue {
                            field: new_field,
                            value: String::new(),
                        })
                    }
                }
            });
            ui.end_row();
        }

        if let Some(rhs_to_remove) = to_remove {
            query_builder.rhs_rels.remove(&rhs_to_remove.clone());
        }
        if let Some((key, rhs_field_to_remove)) = field_to_remove {
            query_builder.rhs_rels.get_mut(&key);
        }
        // ------------------------------
        ui.end_row();
        // ------------------------------
        ui.end_row();
    }

    /// The interactable, dynamic grid of the fields on the left-hand side of a query to the cozo
    /// database.
    ///
    /// The "+" provides the user with a menu/submenu of node type/node field to add to
    /// the left-hand side of the query. Upon selection the query preview in the LHS panel will
    /// auto-update, and the changes will be reflected in the full, non-interactable query preview
    /// in the rightmost panel that contains both the LHS and RHS just as they would be composed in
    /// the query sent to the cozo database using `run_script`.
    // TODO: Figure out how to get indentation to work correctly, with the goal being to make the
    // section look like:
    // ?[
    //      field_one,
    //      field_two,
    //      etc.
    // ]
    fn lhs_grid(&mut self, ui: &mut egui::Ui) {
        let query_builder = &mut self.app_query_builder.query_builder;
        let builder_preview = &mut self.app_query_builder.current_builder_query;

        // ------------------------------
        // ------------------------------
        show_code(ui, "?[");
        ui.menu_button("+", |ui| {
            add_builder_lhs_field(query_builder, builder_preview, ui);
        });
        ui.end_row();
        // ------------------------------
        let mut to_remove = None;
        for lhs_field in query_builder.lhs.iter() {
            ui.horizontal(|ui| {
                ui.label("\t");
                show_code(ui, lhs_field);
                ui.label(",");
            });
            if ui.button("-").clicked() {
                to_remove = Some(lhs_field.clone());
            }
            ui.end_row()
        }
        if let Some(lhs_to_remove) = to_remove {
            query_builder.lhs.remove(lhs_to_remove);
        }

        let mut to_remove: Option<usize> = None;
        for (i, binding) in query_builder.custom_lhs.iter_mut().enumerate() {
            ui.horizontal(|ui| {

        ui.add(TextEdit::singleline(binding));
                ui.text_edit_singleline(binding);
                if ui.button("-").clicked() {
                    to_remove = Some(i);
                }
            });
        }
        if let Some(binding_index) = to_remove {
            query_builder.custom_lhs.remove(binding_index);
        }
        // ------ Custom Bindings -------
        if ui.button("+ Custom").clicked() {
            query_builder.selected_node = None;
            query_builder.insert_lhs_custom(String::new()); // Add new empty binding
        }
        // ------------------------------
        show_code(ui, "]");
        ui.end_row();
        // ------------------------------
        ui.heading("Example");
        ui.end_row();
    }

    /// Column that displays each node type in an interactable button that can be pressed to add
    /// elements related to that column to the query builder.
    /// Should be displayed to the far left of the screen.
    fn render_relation_type_col(&mut self, ui: &mut egui::Ui) {
        ui.push_id("selectable_node_type", |ui| {
            ui.vertical(|ui| {
                ui.label("Node Types");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    use ploke_db::NodeType;
                    for node_type in NodeType::all_variants().iter() {
                        if ui.button(node_type.relation_str()).clicked() {
                            match self.selected_anchor {
                                Anchor::QueryCustom => {
                                    self.app_query_custom
                                        .custom_query
                                        .push_str(node_type.to_base_query());
                                }
                                Anchor::QueryBuilder => {
                                    self.app_query_builder
                                        .current_builder_query
                                        .push_str(node_type.to_base_query());
                                }
                            }
                        }
                    }
                });
            });
        });
    }
    fn render_schema_select_col(&mut self, ui: &mut egui::Ui) {
        ui.push_id("selectable_schema", |ui| {
            ui.vertical(|ui| {
                ui.label("Schema");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    use ploke_db::NodeType;
                    for node_type in NodeType::all_variants().iter() {
                        if ui.button(node_type.relation_str()).clicked() {
                            self.app_query_custom.selected_schema = node_type.to_base_query();
                        }
                    }
                });
            });
        });
    }

    fn render_schema_view(&self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label("Schema View");
                if ui.button("Copy").clicked() {
                    ctx.copy_text(self.app_query_custom.selected_schema.to_string());
                }
            });
            ui.separator();
            ui.label(egui::RichText::new(self.app_query_custom.selected_schema).code());
        });
    }

    fn render_custom_query(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::SidePanel::left("node_types")
            .resizable(true) // changed to false
            .min_width(100.0)
            .default_width(100.0)
            .show_separator_line(true)
            .show_inside(ui, |ui| {
                self.render_schema_select_col(ui);
            });
        egui::SidePanel::right("schema_view")
            .resizable(true) // changed to false
            .min_width(100.0)
            .default_width(200.0)
            .show_separator_line(true)
            .show_inside(ui, |ui| {
                self.render_schema_view(ctx, ui);
            });

        if let Ok(cells) = self.cells.try_borrow_mut() {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // ui.vertical(|ui| {
                    ui.push_id("custom_query_text", |ui| {
                        ui.label("Query:");

                        ui.add(
                            TextEdit::multiline(&mut self.app_query_custom.custom_query)
                                .font(TextStyle::Monospace)
                                .lock_focus(true)
                                .desired_rows(10),
                        );
                        // ui.code_editor(&mut self.app_query_custom.custom_query);
                    });
                    // });
                });
                ui.separator();

                ui.vertical(|ui| {
                    ui.push_id("selected_cells", |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let results = self.results.borrow();

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
                            }
                        });
                    });
                });
            });
        }
    }

    fn render_db_results(&mut self, ui: &mut egui::Ui) {
        use egui_extras::{Size, StripBuilder};

                ui.label("Results:");
                if let Some(duration) = self.last_query_time {
                    ui.label(format!("(Query took: {:.2?})", duration));
                }
        StripBuilder::new(ui)
            .size(Size::remainder().at_least(100.0)) // for the table
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    egui::ScrollArea::horizontal().show(ui, |ui| {
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
                });
            });
    }
}

fn add_builder_lhs_field(
    query_builder: &mut ploke_db::QueryBuilder,
    builder_preview: &mut String,
    ui: &mut egui::Ui,
) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        for relation in NodeType::all_variants() {
            let _menu_item = ui.menu_button(relation.relation_str(), |ui| {
                for field in relation.fields() {
                    if ui.button(*field).clicked() {
                        // NOTE: Kind of a hack, might just want to remove the
                        // `selected_node` from the QueryBuilder
                        query_builder.selected_node = Some(relation);
                        query_builder.insert_lhs_field(field);
                        *builder_preview = query_builder.lhs_to_query_string();
                        log::info!(target: "query-builder",
                            "{} {}\n\t{} {}\n\t{} {:#?}",
                            "adding field".log_step(),
                            field.log_name(),
                            "query_builder.selected_node: ".log_step(),
                            relation.log_name_debug(),
                            "now query_builder.lhs is:".log_step(),
                            query_builder.lhs,

                        );
                    }
                }
                if ui.button("Close the menu").clicked() {
                    ui.close_menu();
                }
            });
        }
    });
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

        egui::TopBottomPanel::top("main_ui_area").show(ctx, |ui| {
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

            // Content area
            ui.horizontal(|ui| {
                self.query_bar_contents(ui, frame, &mut cmd);
            });
            ui.horizontal(|ui| {
                self.query_section_id = Some(ui.id());
            });

            if ui.button("Execute").clicked() {
                self.reset_selected_cells();
                self.execute_query();
            }
        });

        #[cfg(feature = "strip_table")]
        // egui::TopBottomPanel::bottom("results_section").show(ctx, |ui| {
        egui::TopBottomPanel::bottom("query_results")
            .resizable(true)
            .min_height(100.0)
            .show(ctx, |ui| {
                self.render_db_results(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Query Builder");
            // ui.separator();

            match self.selected_anchor {
                Anchor::QueryCustom => self.render_custom_query(ctx, ui),
                Anchor::QueryBuilder => {
                    egui::SidePanel::right("query_preview")
                        .resizable(true)
                        .min_width(100.0)
                        .show_separator_line(true)
                        .show_inside(ui, |ui| {
                            ui.heading("Test Right Area");
                            if ui.button("update").clicked() {
                                let lhs =
                                    self.app_query_builder.query_builder.lhs_to_query_string();
                                let rhs =
                                    self.app_query_builder.query_builder.rhs_to_query_string();
                                self.app_query_builder.current_builder_query =
                                    format!("{} := {}", lhs, rhs);
                            }
                            show_code(ui, &self.app_query_builder.current_builder_query);
                        });
                    egui::CentralPanel::default().show_inside(ui, |ui| {
                        self.render_query_builder_v2(ui);
                        // self.render_query_builder(ui);
                    });
                }
            }
        });

        // NOTE: This is the best currently working version of the query panel area
        // Show the query panel with proper resizing
        // egui::TopBottomPanel::top("query")
        //     .resizable(true)
        //     .min_height(200.0)
        //     .default_height(300.0)
        //     .show_separator_line(true)
        //     .show(ctx, |ui| {
        //         self.show_selected_app(ctx, frame);
        //     });

        #[cfg(not(feature = "strip_table"))]
        egui::TopBottomPanel::bottom("results_section").show(ctx, |ui| {
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
                                                cells
                                                    .selected_cells
                                                    .contains(&(row_index, col_index))
                                            };

                                            // Create a frame with background color if selected
                                            let frame = if is_selected {
                                                egui::Frame::NONE
                                                    .fill(egui::Color32::from_rgb(70, 130, 180))
                                                    .inner_margin(egui::Margin::same(2))
                                            } else {
                                                egui::Frame::NONE
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
                                                    self.modify_table_cell(|cells| {
                                                        if cells.selected_cells.contains(&cell) {
                                                            cells
                                                                .selected_cells
                                                                .retain(|&c| c != cell);
                                                        } else {
                                                            cells.selected_cells.push(cell);
                                                        }
                                                    });
                                                }

                                                // Handle drag start
                                                if response.drag_started() {
                                                    self.modify_table_cell(|cells| {
                                                        cells.selection_in_progress =
                                                            Some((row_index, col_index));
                                                    });
                                                }

                                                // Handle ongoing drag
                                                if response.dragged() {
                                                    self.modify_table_cell(|cells| {
                                                        cells.selection_in_progress = None;
                                                    });
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

    // TODO: Implement this
    #[allow(dead_code, reason = "useful soon")]
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

    fn show_selected_app(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let selected_anchor = self.selected_anchor;
        for (_name, anchor, app) in self.apps_iter_mut() {
            if anchor == selected_anchor || ctx.memory(|mem| mem.everything_is_visible()) {
                app.update(ctx, frame);
            }
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

fn push_selected(cell: (usize, usize), cells: &mut TableCells) {
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

fn show_code(ui: &mut egui::Ui, code: &str) {
    let code = remove_leading_indentation(code.trim_start_matches('\n'));
    crate::rust_view_ui(ui, &code);
}

fn remove_leading_indentation(code: &str) -> String {
    fn is_indent(c: &u8) -> bool {
        matches!(*c, b' ' | b'\t')
    }

    let first_line_indent = code.bytes().take_while(is_indent).count();

    let mut out = String::new();

    let mut code = code;
    while !code.is_empty() {
        let indent = code.bytes().take_while(is_indent).count();
        let start = first_line_indent.min(indent);
        let end = code
            .find('\n')
            .map_or_else(|| code.len(), |endline| endline + 1);
        out += &code[start..end];
        code = &code[end..];
    }
    out
}

/// View some Rust code with syntax highlighting and selection.
pub(crate) fn rust_view_ui(ui: &mut egui::Ui, code: &str) {
    let language = "rs";
    let theme = egui_extras::syntax_highlighting::CodeTheme::from_memory(ui.ctx(), ui.style());
    egui_extras::syntax_highlighting::code_view_ui(ui, &theme, code, language);
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Ploke UI",
        options,
        Box::new(|_cc| Ok(Box::new(PlokeApp::new()))),
    )
}
