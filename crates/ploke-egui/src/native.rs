//! Native entry point for the operator graph UI.

use std::error::Error;
use std::path::PathBuf;

use crate::cli::Run;
#[cfg(feature = "dev")]
use crate::cli::report::{
    print_artifact_edges_report, print_artifact_ids_report, print_node_inspector,
};
use crate::demo::sample_graph;
#[cfg(feature = "dev")]
use crate::diagnostics::{
    GraphIdentity, RunSnapshot, Snapshot, SnapshotObservation, SnapshotSink,
    artifact_component_breakdown,
};
use crate::import::graph_from_run_root;
#[cfg(all(feature = "dev", feature = "profile-with-puffin"))]
use crate::perf::PuffinCapture;
#[cfg(feature = "dev")]
use crate::perf::{PerformanceLogSink, PerformanceRun};
use crate::run_picker::RunPicker;
use crate::ui::app::{OperatorApp, layout};
#[cfg(feature = "dev")]
use crate::ui::view::GraphView;
use crate::ui::view::GraphViewMode;
#[cfg(feature = "dev")]
use eframe::egui::Vec2;
use eframe::egui::ViewportBuilder;
use ploke_tree::Graph;

pub fn run() -> Result<(), Box<dyn Error>> {
    profiling::register_thread!("ploke-egui.main");
    let run = Run::from_env();
    let explicit_run_root = run.run_root.clone();
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([layout::DEFAULT_WINDOW_WIDTH, layout::DEFAULT_WINDOW_HEIGHT]),
        ..Default::default()
    };
    let mut picker = RunPicker::from_default_root();
    let initial_run_root = explicit_run_root
        .clone()
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
    #[cfg(feature = "dev")]
    if run.perf_log {
        let Some(run_root) = explicit_run_root.clone() else {
            return Err("--perf-log requires an explicit --run-root or positional RUN_ROOT".into());
        };
        let sink = PerformanceLogSink::new(default_profiling_dir())?;
        let path = sink.observe(PerformanceRun::new(
            run_root,
            run.mode,
            Vec2::new(
                layout::DEFAULT_CENTER_CANVAS_WIDTH,
                layout::DEFAULT_CENTER_CANVAS_HEIGHT,
            ),
        ))?;
        println!("Performance log written: {}", path.display());
        return Ok(());
    }
    #[cfg(feature = "dev")]
    if run.artifact_edges_report {
        let graph = initial_graph(initial_run_root.clone())?;
        print_artifact_edges_report(&graph)?;
        return Ok(());
    }
    let graph = initial_graph(initial_run_root.clone())?;
    #[cfg(feature = "dev")]
    if run.contract_report {
        print_contract_report(&graph, run.mode, initial_run_root.as_deref())?;
        return Ok(());
    }
    #[cfg(feature = "dev")]
    if let Some(selector) = run.inspect_node.as_deref() {
        print_node_inspector(&graph, selector)?;
        return Ok(());
    }
    #[cfg(feature = "dev")]
    if let Some(selector) = run.artifact_ids_report.as_deref() {
        print_artifact_ids_report(&graph, selector)?;
        return Ok(());
    }
    #[cfg(all(feature = "dev", not(feature = "profile-with-puffin")))]
    if run.puffin_capture_frames.is_some() {
        return Err("--puffin-capture-frames requires --features profile-with-puffin".into());
    }

    let app = app(graph, run.mode, run.snapshot, picker)?;
    #[cfg(all(feature = "dev", feature = "profile-with-puffin"))]
    let app = if let Some(frame_target) = run.puffin_capture_frames {
        let Some(run_root) = explicit_run_root.clone() else {
            return Err(
                "--puffin-capture-frames requires an explicit --run-root or positional RUN_ROOT"
                    .into(),
            );
        };
        app.with_puffin_capture(PuffinCapture::new(
            frame_target,
            default_puffin_capture_dir(),
            run_root,
            run.mode,
            run.puffin_capture_close,
        )?)
    } else {
        app
    };
    eframe::run_native("ploke-egui", options, Box::new(|_cc| Ok(Box::new(app))))?;
    Ok(())
}

fn initial_graph(run_root: Option<PathBuf>) -> Result<Graph, Box<dyn Error>> {
    profiling::scope!("ploke-egui.initial-graph");
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
fn print_contract_report(
    graph: &Graph,
    mode: GraphViewMode,
    run_root: Option<&std::path::Path>,
) -> Result<(), Box<dyn Error>> {
    profiling::scope!("ploke-egui.contract-report");
    let diagnostics = GraphView::contract_diagnostics(
        graph,
        mode,
        Vec2::new(
            layout::DEFAULT_CENTER_CANVAS_WIDTH,
            layout::DEFAULT_CENTER_CANVAS_HEIGHT,
        ),
    )
    .ok_or_else(|| {
        format!(
            "could not produce contract diagnostics for mode {}",
            mode.as_str()
        )
    })?;
    let run = run_root.map(run_snapshot);
    let graph_identity = GraphIdentity::from_graph(graph, &diagnostics);
    let observation = SnapshotObservation::new(diagnostics)
        .with_graph_has_content(
            graph.history.blocks.len()
                + graph.candidates.candidates.len()
                + graph.artifacts.artifacts.len()
                + graph.selections.selections.len()
                + graph.runtimes.runtimes.len()
                + graph.operations.operations.len()
                + graph.evidence.attachments.len()
                > 0,
        )
        .with_run(run)
        .with_graph_identity(graph_identity)
        .with_artifact_components(artifact_component_breakdown(graph));
    let snapshot = Snapshot::from_observation(1, observation);
    print!("{}", snapshot.render_text());
    Ok(())
}

#[cfg(feature = "dev")]
fn run_snapshot(path: &std::path::Path) -> RunSnapshot {
    RunSnapshot {
        name: path
            .parent()
            .and_then(std::path::Path::file_name)
            .or_else(|| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string()),
        path: path.display().to_string(),
    }
}

#[cfg(feature = "dev")]
fn default_diagnostics_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/diagnostics")
}

#[cfg(feature = "dev")]
fn default_profiling_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/profiling")
}

#[cfg(all(feature = "dev", feature = "profile-with-puffin"))]
fn default_puffin_capture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/profiling/puffin")
}
