use std::fmt::Write as _;

use super::Report;

const COMPONENT_TEXT_LIMIT: usize = 20;
const ITEM_TEXT_LIMIT: usize = 6;

impl Report<'_> {
    pub(crate) fn render_text_into(&self, out: &mut String) {
        let _ = writeln!(out, "default-view contract:");
        let _ = writeln!(
            out,
            "layout: top={}, left={}, center={}, right={}, bottom={}",
            self.layout.top_strip_present,
            self.layout.left_sidebar_present,
            self.layout.center_canvas_present,
            self.layout.right_inspector_present,
            self.layout.bottom_timeline_present
        );
        let _ = writeln!(
            out,
            "layout widths: default_window={}px, left_sidebar={}px, left_sidebar_max={}px, right_inspector={}px, right_inspector_max={}px, center_canvas={}px, min_center_canvas={}px, center_canvas={}%, min_center_canvas={}%, passes={}",
            self.layout.width_budget.default_window_width_logical_px,
            self.layout.width_budget.left_sidebar_width_logical_px,
            self.layout.width_budget.left_sidebar_max_width_logical_px,
            self.layout.width_budget.right_inspector_width_logical_px,
            self.layout
                .width_budget
                .right_inspector_max_width_logical_px,
            self.layout.width_budget.center_canvas_width_logical_px,
            self.layout.width_budget.min_center_canvas_width_logical_px,
            self.layout.width_budget.center_canvas_width_percent,
            self.layout.width_budget.min_center_canvas_width_percent,
            self.layout.width_budget.center_canvas_satisfies_minimum()
        );
        if let Some(identity) = &self.graph_identity {
            let _ = writeln!(
                out,
                "graph identity: F_nodes={}, F_roots={}, visible_nodes={}, visible_edges={}, A_nodes={}, P_H={}, P_C={}, P_O={}, P_B={}, visible_fingerprint={}",
                identity.forest_nodes,
                identity.forest_roots,
                identity.default_visible_nodes,
                identity.default_visible_edges,
                identity.artifact_tree_nodes,
                identity.artifact_tree_p_h,
                identity.artifact_tree_p_c,
                identity.artifact_tree_p_o,
                identity.artifact_tree_p_b,
                identity.visible_node_fingerprint
            );
            let _ = writeln!(
                out,
                "graph identity keys: [{}]{}",
                summarize_items(&identity.visible_node_keys_preview),
                if identity.visible_node_keys_truncated == 0 {
                    String::new()
                } else {
                    format!(" +{}", identity.visible_node_keys_truncated)
                }
            );
        } else {
            let _ = writeln!(out, "graph identity: not_recorded");
        }
        let _ = writeln!(
            out,
            "controls: run_selector={}, mode_selector={}, load_state={:?}, quick_filters={}, hide_unconsidered_children={}",
            self.controls.run_selector_present,
            self.controls.mode_selector_present,
            self.controls.load_state,
            self.controls.quick_filters_present,
            self.controls.hide_unconsidered_children
        );
        let _ = writeln!(
            out,
            "eval protocol: closure={}, run_records={}, protocol_artifacts={}",
            self.eval_protocol.closure.as_str(),
            self.eval_protocol.run_records.as_str(),
            self.eval_protocol.protocol_artifacts.as_str()
        );
        let _ = writeln!(
            out,
            "center: mode={}, nodes={}, edges={}, hidden_edges={}, synthetic_anchors_visible={}",
            self.center.mode,
            self.center.visible_node_count,
            self.center.visible_edge_count,
            self.center.hidden_edge_count,
            self.center.synthetic_anchors_visible
        );
        if self.center.nodes.f > 0 {
            let _ = writeln!(
                out,
                "center node sets: F={}, A={}",
                self.center.nodes.f, self.center.nodes.a
            );
            let _ = writeln!(
                out,
                "center edge sets: E_F={}, P_H={}, P_C={}, visible_total={}, P_O_context={}, P_B_context={}",
                self.center.edges.e_f,
                self.center.edges.p_h,
                self.center.edges.p_c,
                self.center.edges.visible_primary_total(),
                self.center.edges.p_o,
                self.center.edges.p_b
            );
        } else {
            let _ = writeln!(out, "center node sets: A={}", self.center.nodes.a);
            let _ = writeln!(
                out,
                "center edge sets: P_H={}, P_C={}, visible_primary={}, P_O_context={}, P_B_context={}",
                self.center.edges.p_h,
                self.center.edges.p_c,
                self.center.edges.visible_primary_total(),
                self.center.edges.p_o,
                self.center.edges.p_b
            );
        }
        let _ = writeln!(
            out,
            "center components: weak={}, roots={}, orphan_artifacts={}, weakly_connected={}",
            self.center.components.weak,
            self.center.components.roots,
            self.center.components.orphan_artifacts,
            self.center.components.weakly_connected
        );
        if !self.center.component_breakdown.is_empty() {
            let _ = writeln!(out, "component breakdown:");
            for component in self
                .center
                .component_breakdown
                .iter()
                .take(COMPONENT_TEXT_LIMIT)
            {
                let _ = writeln!(
                    out,
                    "- #{}: roots=[{}], artifacts={} [{}], P_H={}, P_C={}, P_O={}, P_B={}",
                    component.index,
                    summarize_items(&component.roots),
                    component.artifacts.len(),
                    summarize_items(&component.artifacts),
                    component.p_h.len(),
                    component.p_c.len(),
                    component.p_o.len(),
                    component.p_b.len()
                );
                for edge in component.p_h.iter().take(ITEM_TEXT_LIMIT) {
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
                for edge in component.p_o.iter().take(ITEM_TEXT_LIMIT) {
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
                for edge in component.p_c.iter().take(ITEM_TEXT_LIMIT) {
                    let _ = writeln!(
                        out,
                        "  P_C {} -> {} via {}",
                        edge.from,
                        edge.to,
                        edge.sources
                            .first()
                            .map(|source| source.node_id.as_str())
                            .unwrap_or("unknown-child")
                    );
                }
                for edge in component.p_b.iter().take(ITEM_TEXT_LIMIT) {
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
            if self.center.component_breakdown.len() > COMPONENT_TEXT_LIMIT {
                let _ = writeln!(
                    out,
                    "- ... {} more components in JSON",
                    self.center.component_breakdown.len() - COMPONENT_TEXT_LIMIT
                );
            }
        }
        let _ = writeln!(
            out,
            "center marks: ruler_highlights={}, dimmed_children={}, dotted_child_edges={}",
            self.center.marks.ruler_highlights,
            self.center.marks.dimmed_children,
            self.center.marks.dotted_child_edges
        );
        if let Some(selected) = &self.inspector.selected_detail {
            let _ = writeln!(out, "selection: {} {}", selected.kind, selected.label);
        } else {
            let _ = writeln!(out, "selection: none");
        }
        if let Some(inspector) = &self.inspector.selected_inspector {
            let identity_count = usize::from(inspector.identity.is_some());
            let metric_count = usize::from(inspector.metrics.is_some());
            let unavailable_count = usize::from(inspector.unavailable.is_some());
            let _ = writeln!(
                out,
                "selection inspector: identity={}, roles={}, metrics={}, incoming={}, outgoing={}, artifact_incoming={}, artifact_outgoing={}, source_refs={}, unavailable={}",
                identity_count,
                inspector.roles.len(),
                metric_count,
                inspector.incoming.len(),
                inspector.outgoing.len(),
                inspector.artifact_incoming.len(),
                inspector.artifact_outgoing.len(),
                inspector.source_refs.len(),
                unavailable_count
            );
            if let Some(identity) = &inspector.identity {
                match identity {
                    crate::ui::inspector::SelectionIdentity::RunForestNode(identity) => {
                        let _ = writeln!(out, "- identity.run_forest_node={}", identity.node_key);
                        let _ = writeln!(out, "- identity.candidate={}", identity.candidate_id);
                        let _ = writeln!(
                            out,
                            "- identity.source_artifact={}",
                            identity.source_artifact
                        );
                        if let Some(parent) = identity.parent_node {
                            let _ = writeln!(out, "- identity.parent_run_forest_node={parent}");
                        }
                    }
                    crate::ui::inspector::SelectionIdentity::Artifact(identity) => {
                        let _ = writeln!(out, "- identity.artifact={}", identity.artifact);
                    }
                }
            }
            if let Some(metrics) = &inspector.metrics {
                match metrics {
                    crate::ui::inspector::SelectionMetrics::RunForestNode(metrics) => {
                        let _ = writeln!(out, "- metric.generation={}", metrics.generation);
                        let _ = writeln!(
                            out,
                            "- metric.child_run_forest_nodes={}",
                            metrics.child_run_forest_nodes
                        );
                    }
                    crate::ui::inspector::SelectionMetrics::Artifact(metrics) => {
                        let _ = writeln!(out, "- metric.source_records={}", metrics.source_records);
                        let _ = writeln!(out, "- metric.evidence_refs={}", metrics.evidence_refs);
                    }
                }
            }
            for edge in inspector.incoming.iter().take(4) {
                let _ = writeln!(
                    out,
                    "- incoming.{}: {} -> {} ({})",
                    edge.relation.label(),
                    edge.from,
                    edge.to,
                    edge.source_count
                );
            }
            for edge in inspector.outgoing.iter().take(4) {
                let _ = writeln!(
                    out,
                    "- outgoing.{}: {} -> {} ({})",
                    edge.relation.label(),
                    edge.from,
                    edge.to,
                    edge.source_count
                );
            }
            for edge in inspector.artifact_incoming.iter().take(4) {
                let _ = writeln!(
                    out,
                    "- artifact_incoming.{}: {} -> {} ({})",
                    edge.relation.label(),
                    edge.from,
                    edge.to,
                    edge.source_count
                );
            }
            for edge in inspector.artifact_outgoing.iter().take(4) {
                let _ = writeln!(
                    out,
                    "- artifact_outgoing.{}: {} -> {} ({})",
                    edge.relation.label(),
                    edge.from,
                    edge.to,
                    edge.source_count
                );
            }
            for source_ref in inspector.source_refs.iter().take(4) {
                let _ = writeln!(out, "- source_ref={source_ref}");
            }
            if let Some(reason) = inspector.unavailable {
                let _ = writeln!(out, "- unavailable.{}={}", reason.subject(), reason.state());
            }
        }
        let _ = writeln!(
            out,
            "inspector: present={}, record_refs={}, drilldowns={}, unavailable_reasons={}",
            self.inspector.right_inspector_present,
            self.inspector.record_refs_present,
            self.inspector.drilldown_candidates_present,
            self.inspector.unavailable_reason_classified
        );
        let _ = writeln!(
            out,
            "timeline: present={}, compact={}, synced_selection={}, spans_reported={}, order_strength_reported={}",
            self.timeline.bottom_timeline_present,
            self.timeline.compact,
            self.timeline.synced_selection,
            self.timeline.spans_reported,
            self.timeline.order_strength_reported
        );
        let _ = writeln!(out, "checks:");
        for check in &self.checks {
            let _ = writeln!(out, "- {:?}: {}", check.status, check.id);
            if !check.evidence.is_empty() {
                let _ = writeln!(out, "  {}", check.evidence.join("; "));
            }
        }
        let _ = writeln!(out);
    }
}

fn summarize_items(items: &[String]) -> String {
    let mut text = items
        .iter()
        .take(ITEM_TEXT_LIMIT)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    if items.len() > ITEM_TEXT_LIMIT {
        let _ = write!(text, ", ... +{}", items.len() - ITEM_TEXT_LIMIT);
    }
    text
}
