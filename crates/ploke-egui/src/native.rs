//! Native entry point for the operator graph UI.

use std::error::Error;
use std::path::PathBuf;

use crate::cli::Run;
use crate::demo::sample_graph;
#[cfg(feature = "dev")]
use crate::diagnostics::{Snapshot, SnapshotObservation, SnapshotSink};
use crate::import::graph_from_run_root;
use crate::run_picker::RunPicker;
use crate::ui::app::{OperatorApp, layout};
use crate::ui::view::{GraphView, GraphViewMode};
use eframe::egui::{Vec2, ViewportBuilder};
use ploke_tree::Graph;

pub fn run() -> Result<(), Box<dyn Error>> {
    let run = Run::from_env();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([layout::DEFAULT_WINDOW_WIDTH, layout::DEFAULT_WINDOW_HEIGHT]),
        ..Default::default()
    };
    let mut picker = RunPicker::from_default_root();
    let initial_run_root = run
        .run_root
        .or_else(|| picker.first_loadable_path().map(PathBuf::from));
    if let Some(path) = initial_run_root.as_deref() {
        picker.select_path(path);
    }
    #[cfg(feature = "dev")]
    if run.run_picker_report {
        print!("{}", picker.diagnostics().render_text());
        return Ok(());
    }
    #[cfg(feature = "dev")]
    if run.artifact_connectivity_report {
        print!("{}", picker.artifact_connectivity_batch().render_text());
        return Ok(());
    }
    let graph = initial_graph(initial_run_root)?;
    #[cfg(feature = "dev")]
    if run.contract_report {
        print_contract_report(&graph, run.mode)?;
        return Ok(());
    }
    let app = app(graph, run.mode, run.snapshot, picker)?;
    eframe::run_native("ploke-egui", options, Box::new(|_cc| Ok(Box::new(app))))?;
    Ok(())
}

fn initial_graph(run_root: Option<PathBuf>) -> Result<Graph, Box<dyn Error>> {
    let Some(run_root) = run_root else {
        return Ok(sample_graph());
    };

    Ok(graph_from_run_root(run_root)?)
}

fn app(
    graph: Graph,
    mode: GraphViewMode,
    snapshot: bool,
    picker: RunPicker,
) -> Result<OperatorApp, Box<dyn Error>> {
    let app = OperatorApp::new_with_run_picker(graph, picker).with_mode(mode);
    #[cfg(feature = "dev")]
    {
        if !snapshot {
            return Ok(app);
        }
        return Ok(app.with_snapshot_sink(SnapshotSink::new(default_diagnostics_dir())?));
    }

    #[cfg(not(feature = "dev"))]
    {
        let _ = snapshot;
        Ok(app)
    }
}

#[cfg(feature = "dev")]
fn print_contract_report(graph: &Graph, mode: GraphViewMode) -> Result<(), Box<dyn Error>> {
    let diagnostics = GraphView::contract_diagnostics(
        graph,
        mode,
        Vec2::new(
            layout::DEFAULT_CENTER_CANVAS_WIDTH,
            layout::DEFAULT_WINDOW_HEIGHT,
        ),
    )
    .ok_or_else(|| {
        format!(
            "could not produce contract diagnostics for mode {}",
            mode.as_str()
        )
    })?;
    let observation = SnapshotObservation::new(diagnostics).with_graph_has_content(
        graph.history.blocks.len()
            + graph.candidates.candidates.len()
            + graph.artifacts.artifacts.len()
            + graph.selections.selections.len()
            + graph.runtimes.runtimes.len()
            + graph.operations.operations.len()
            + graph.evidence.attachments.len()
            > 0,
    );
    let snapshot = Snapshot::from_observation(1, observation);
    print!("{}", snapshot.render_text());
    Ok(())
}

#[cfg(feature = "dev")]
fn default_diagnostics_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/diagnostics")
}
