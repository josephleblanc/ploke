//! Text reports for dev-only CLI inspection surfaces.

use std::error::Error;
use std::fmt::Write as _;

use ploke_tree::Graph;

use crate::diagnostics::artifact_component_breakdown;
use crate::ui::inspector::SelectionInspector;

pub fn print_node_inspector(graph: &Graph, selector: &str) -> Result<(), Box<dyn Error>> {
    let (selection, inspector) = SelectionInspector::from_default_selector(graph, selector)
        .ok_or_else(|| format!("no visible default graph node matches '{selector}'"))?;
    print!("{}", inspector.snapshot(&selection).render_text());
    Ok(())
}

pub fn print_artifact_ids_report(graph: &Graph, selector: &str) -> Result<(), Box<dyn Error>> {
    let (selection, inspector) = SelectionInspector::from_default_selector(graph, selector)
        .ok_or_else(|| format!("no visible default graph node matches '{selector}'"))?;
    println!("selection: {} {}", selection.kind, selection.label);
    print!("{}", inspector.artifact_ids_section().render_text());
    Ok(())
}

pub fn print_artifact_edges_report(graph: &Graph) -> Result<(), Box<dyn Error>> {
    let components = artifact_component_breakdown(graph);
    let mut out = String::new();
    let _ = writeln!(out, "artifact-edge report:");
    let _ = writeln!(out, "components: {}", components.len());
    for component in &components {
        let _ = writeln!(
            out,
            "- #{}: roots=[{}], artifacts={} [{}], P_H={}, P_O={}, P_B={}",
            component.index,
            component.roots.join(", "),
            component.artifacts.len(),
            component.artifacts.join(", "),
            component.p_h.len(),
            component.p_o.len(),
            component.p_b.len()
        );
        for edge in &component.p_h {
            let _ = writeln!(
                out,
                "  P_H {} -> {} via {}",
                edge.from,
                edge.to,
                edge.sources
                    .first()
                    .map(|source| source.block_hash.as_str())
                    .unwrap_or("unknown-block")
            );
        }
        for edge in &component.p_o {
            let _ = writeln!(
                out,
                "  P_O {} -> {} via {}",
                edge.from,
                edge.to,
                edge.sources
                    .first()
                    .map(|source| source.block_hash.as_str())
                    .unwrap_or("unknown-block")
            );
        }
        for edge in &component.p_b {
            let _ = writeln!(
                out,
                "  P_B {} -> {} via {}",
                edge.from,
                edge.to,
                edge.sources
                    .first()
                    .map(|source| source.branch_id.as_str())
                    .unwrap_or("unknown-branch")
            );
        }
    }
    print!("{out}");
    Ok(())
}
