//! Native entry point for the operator graph UI.

use std::error::Error;
use std::path::PathBuf;

use crate::demo::sample_graph;
use crate::graph::Graph;
use crate::import::graph_from_run_root;
use crate::ui::app::OperatorApp;

pub fn run() -> Result<(), Box<dyn Error>> {
    let options = eframe::NativeOptions::default();
    let graph = initial_graph()?;
    eframe::run_native(
        "ploke-egui",
        options,
        Box::new(|_cc| Ok(Box::new(OperatorApp::new(graph)))),
    )?;
    Ok(())
}

fn initial_graph() -> Result<Graph, Box<dyn Error>> {
    let Some(run_root) = std::env::args_os().nth(1) else {
        return Ok(sample_graph());
    };

    Ok(graph_from_run_root(PathBuf::from(run_root))?)
}
