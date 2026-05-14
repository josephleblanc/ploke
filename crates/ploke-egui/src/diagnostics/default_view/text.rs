use std::fmt::Write as _;

use super::Report;

impl Report {
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
            "layout widths: default_window={}px, left_sidebar={}px, left_sidebar_max={}px, center_canvas={}px, min_center_canvas={}px, center_canvas={}%, min_center_canvas={}%, passes={}",
            self.layout.width_budget.default_window_width_logical_px,
            self.layout.width_budget.left_sidebar_width_logical_px,
            self.layout.width_budget.left_sidebar_max_width_logical_px,
            self.layout.width_budget.center_canvas_width_logical_px,
            self.layout.width_budget.min_center_canvas_width_logical_px,
            self.layout.width_budget.center_canvas_width_percent,
            self.layout.width_budget.min_center_canvas_width_percent,
            self.layout.width_budget.center_canvas_satisfies_minimum()
        );
        let _ = writeln!(
            out,
            "controls: run_selector={}, mode_selector={}, load_state={:?}, quick_filters={}",
            self.controls.run_selector_present,
            self.controls.mode_selector_present,
            self.controls.load_state,
            self.controls.quick_filters_present
        );
        let _ = writeln!(
            out,
            "center: mode={}, nodes={}, edges={}, synthetic_anchors_visible={}",
            self.center.mode,
            self.center.visible_node_count,
            self.center.visible_edge_count,
            self.center.synthetic_anchors_visible
        );
        let _ = writeln!(out, "center node sets: A={}", self.center.nodes.a);
        let _ = writeln!(
            out,
            "center edge sets: P_H={}, P_B={}, P={}",
            self.center.edges.p_h,
            self.center.edges.p_b,
            self.center.edges.p()
        );
        let _ = writeln!(
            out,
            "center components: weak={}, roots={}, orphan_artifacts={}, weakly_connected={}",
            self.center.components.weak,
            self.center.components.roots,
            self.center.components.orphan_artifacts,
            self.center.components.weakly_connected
        );
        let _ = writeln!(
            out,
            "center marks: ruler_highlights={}",
            self.center.marks.ruler_highlights
        );
        if let Some(selected) = &self.inspector.selected_detail {
            let _ = writeln!(out, "selection: {} {}", selected.kind, selected.label);
        } else {
            let _ = writeln!(out, "selection: none");
        }
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
