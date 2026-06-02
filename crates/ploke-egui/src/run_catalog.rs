//! Run/graph catalog for the left navigation panel.
//!
//! Native loads prototype run directories via [`crate::run_picker::RunPicker`]
//! (rendered separately). WASM loads export-graph JSON via [`crate::bootstrap::GraphCatalog`].
//! Benchmark scenarios may show a compact native catalog placeholder.

use egui::Ui;
use ploke_tree::Graph;

use crate::bootstrap::GraphCatalog;

#[cfg(not(target_arch = "wasm32"))]
use crate::run_picker::RunPicker;

/// Sidebar graph catalog (WASM primary; native benchmark placeholder only).
pub trait RunCatalog {
    fn show(&mut self, ui: &mut Ui, compact: bool) -> Option<Graph>;
    fn current_label(&self) -> Option<&str>;
}

impl RunCatalog for GraphCatalog {
    fn show(&mut self, ui: &mut Ui, compact: bool) -> Option<Graph> {
        GraphCatalog::show(self, ui, compact)
    }

    fn current_label(&self) -> Option<&str> {
        self.selected_label()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct NativeBenchmarkCatalog<'a> {
    pub catalog: &'a mut GraphCatalog,
    pub visible: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl RunCatalog for NativeBenchmarkCatalog<'_> {
    fn show(&mut self, ui: &mut Ui, compact: bool) -> Option<Graph> {
        if self.visible {
            self.catalog.show(ui, compact)
        } else {
            None
        }
    }

    fn current_label(&self) -> Option<&str> {
        self.catalog.selected_label()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn native_run_label<'a>(
    run_picker: &'a RunPicker,
    graph_snapshot_label: Option<&'a str>,
) -> Option<&'a str> {
    run_picker
        .selected_run_name()
        .or(graph_snapshot_label)
        .filter(|name| !name.is_empty())
}
