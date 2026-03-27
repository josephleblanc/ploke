//! Narrow a structural CU slice using `#[cfg]` strings on graph nodes (see `cfg_eval`).

#[cfg(feature = "cfg_eval")]
use crate::parser::graph::{GraphAccess, GraphNode};
#[cfg(feature = "cfg_eval")]
use crate::parser::visitor::{ActiveCfg, parse_cfg_expr_from_inner_tokens};
#[cfg(feature = "cfg_eval")]
use crate::parser::{ParsedCodeGraph, types::TypeNode};
use ploke_core::IdTrait;
#[cfg(feature = "cfg_eval")]
use std::collections::HashSet;
#[cfg(feature = "cfg_eval")]
use uuid::Uuid;

#[cfg(feature = "cfg_eval")]
use super::{EnabledSyntacticEdge, StructuralCompilationUnitSlice};

/// Drop enabled nodes whose own `#[cfg]` strings (when present) evaluate false under `active`,
/// then recompute enabled edges (both endpoints kept) and enabled files (paths still referenced).
///
/// **Limitations:** Matches [`crate::parser::visitor::cfg_evaluator`] (unsupported cfg atoms →
/// false). Does not yet combine module-ancestor cfgs.
#[cfg(feature = "cfg_eval")]
pub fn filter_structural_slice_by_cfg(
    graph: &ParsedCodeGraph,
    slice: &StructuralCompilationUnitSlice,
    active: &ActiveCfg,
) -> StructuralCompilationUnitSlice {
    let enabled_node_ids: HashSet<Uuid> = slice
        .enabled_node_ids
        .iter()
        .copied()
        .filter(|id| node_passes_cfg(graph, *id, active))
        .collect();

    let enabled_edges: Vec<EnabledSyntacticEdge> = slice
        .enabled_edges
        .iter()
        .filter(|e| {
            enabled_node_ids.contains(&e.source_id) && enabled_node_ids.contains(&e.target_id)
        })
        .cloned()
        .collect();

    let enabled_file_paths: HashSet<std::path::PathBuf> = slice
        .enabled_file_paths
        .iter()
        .filter(|p| file_still_reachable(graph, p, &enabled_node_ids))
        .cloned()
        .collect();

    StructuralCompilationUnitSlice {
        key: slice.key.clone(),
        cu_id: slice.cu_id,
        enabled_node_ids,
        enabled_edges,
        enabled_file_paths,
    }
}

#[cfg(feature = "cfg_eval")]
fn node_passes_cfg(graph: &ParsedCodeGraph, id: Uuid, active: &ActiveCfg) -> bool {
    match cfgs_for_uuid(graph, id) {
        None => true,
        Some(strings) if strings.is_empty() => true,
        Some(strings) => strings.iter().all(|s| {
            parse_cfg_expr_from_inner_tokens(s)
                .map(|e| active.eval(&e))
                .unwrap_or(false)
        }),
    }
}

#[cfg(feature = "cfg_eval")]
fn cfgs_for_uuid(graph: &ParsedCodeGraph, id: Uuid) -> Option<&[String]> {
    if let Some(f) = graph.functions().iter().find(|f| f.any_id().uuid() == id) {
        return Some(f.cfgs());
    }
    for t in graph.traits() {
        if t.any_id().uuid() == id {
            return Some(GraphNode::cfgs(t));
        }
        for m in &t.methods {
            if m.any_id().uuid() == id {
                return Some(m.cfgs());
            }
        }
    }
    for i in graph.impls() {
        if i.any_id().uuid() == id {
            return Some(i.cfgs());
        }
        for m in &i.methods {
            if m.any_id().uuid() == id {
                return Some(m.cfgs());
            }
        }
    }
    for def in graph.defined_types() {
        if def.any_id().uuid() == id {
            return Some(GraphNode::cfgs(def));
        }
    }
    for n in graph.modules() {
        if n.any_id().uuid() == id {
            return Some(n.cfgs());
        }
    }
    for n in graph.consts() {
        if n.any_id().uuid() == id {
            return Some(n.cfgs());
        }
    }
    for n in graph.statics() {
        if n.any_id().uuid() == id {
            return Some(n.cfgs());
        }
    }
    for n in graph.macros() {
        if n.any_id().uuid() == id {
            return Some(n.cfgs());
        }
    }
    for n in graph.use_statements() {
        if n.any_id().uuid() == id {
            return Some(n.cfgs());
        }
    }
    for n in graph.unresolved_nodes() {
        if n.any_id().uuid() == id {
            return Some(n.cfgs());
        }
    }
    if graph
        .type_graph()
        .iter()
        .any(|t: &TypeNode| t.id.uuid() == id)
    {
        return None;
    }
    None
}

#[cfg(feature = "cfg_eval")]
fn file_still_reachable(
    graph: &ParsedCodeGraph,
    path: &std::path::Path,
    enabled: &HashSet<Uuid>,
) -> bool {
    graph.modules().iter().any(|m| {
        m.file_path().map(|p| p == path).unwrap_or(false) && enabled.contains(&m.any_id().uuid())
    })
}
