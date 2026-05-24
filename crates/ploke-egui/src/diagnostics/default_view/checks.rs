use crate::ui::view::GraphViewMode;

use super::{
    ArtifactTree, Check, CheckStatus, Controls, EvalProtocolEvidence, Inspector, Layout, LoadState,
    Timeline,
};

pub(super) fn build(
    layout: &Layout,
    controls: &Controls,
    eval_protocol: &EvalProtocolEvidence,
    center: &ArtifactTree,
    inspector: &Inspector<'_>,
    timeline: &Timeline,
) -> Vec<Check> {
    vec![
        check(
            "default-mode-artifact-tree",
            if center.mode == GraphViewMode::ArtifactTree.as_str() {
                CheckStatus::Passed
            } else {
                CheckStatus::Failed
            },
            vec![format!("mode={}", center.mode)],
        ),
        check(
            "left-sidebar-present",
            bool_status(layout.left_sidebar_present),
            vec![format!(
                "left_sidebar_present={}",
                layout.left_sidebar_present
            )],
        ),
        check(
            "center-canvas-present",
            bool_status(layout.center_canvas_present && center.canvas_present),
            vec![
                format!(
                    "layout.center_canvas_present={}",
                    layout.center_canvas_present
                ),
                format!("center.canvas_present={}", center.canvas_present),
            ],
        ),
        check(
            "center-canvas-width-budget",
            bool_status(layout.width_budget.center_canvas_satisfies_minimum()),
            vec![
                format!(
                    "default_window_width_logical_px={}",
                    layout.width_budget.default_window_width_logical_px
                ),
                format!(
                    "left_sidebar_width_logical_px={}",
                    layout.width_budget.left_sidebar_width_logical_px
                ),
                format!(
                    "left_sidebar_max_width_logical_px={}",
                    layout.width_budget.left_sidebar_max_width_logical_px
                ),
                format!(
                    "right_inspector_width_logical_px={}",
                    layout.width_budget.right_inspector_width_logical_px
                ),
                format!(
                    "right_inspector_max_width_logical_px={}",
                    layout.width_budget.right_inspector_max_width_logical_px
                ),
                format!(
                    "center_canvas_width_logical_px={}",
                    layout.width_budget.center_canvas_width_logical_px
                ),
                format!(
                    "min_center_canvas_width_logical_px={}",
                    layout.width_budget.min_center_canvas_width_logical_px
                ),
                format!(
                    "center_canvas_width_percent={}",
                    layout.width_budget.center_canvas_width_percent
                ),
                format!(
                    "min_center_canvas_width_percent={}",
                    layout.width_budget.min_center_canvas_width_percent
                ),
            ],
        ),
        check(
            "run-selector-present",
            bool_status(controls.run_selector_present),
            vec![format!(
                "run_selector_present={}",
                controls.run_selector_present
            )],
        ),
        check(
            "mode-selector-present",
            bool_status(controls.mode_selector_present),
            vec![format!(
                "mode_selector_present={}",
                controls.mode_selector_present
            )],
        ),
        check(
            "eval-protocol-evidence-reported",
            CheckStatus::Passed,
            vec![
                format!("closure={}", eval_protocol.closure.as_str()),
                format!("run_records={}", eval_protocol.run_records.as_str()),
                format!(
                    "protocol_artifacts={}",
                    eval_protocol.protocol_artifacts.as_str()
                ),
            ],
        ),
        check(
            "non-empty-artifact-run-renders-nodes",
            match controls.load_state {
                LoadState::Loaded if center.visible_node_count > 0 => CheckStatus::Passed,
                LoadState::Loaded => CheckStatus::Failed,
                LoadState::Empty | LoadState::Failed => CheckStatus::NotApplicable,
            },
            vec![
                format!("load_state={:?}", controls.load_state),
                format!("visible_node_count={}", center.visible_node_count),
            ],
        ),
        check(
            "synthetic-anchors-hidden",
            bool_status(!center.synthetic_anchors_visible),
            vec![format!(
                "synthetic_anchors_visible={}",
                center.synthetic_anchors_visible
            )],
        ),
        check(
            "center-node-set-reported",
            CheckStatus::Passed,
            vec![
                format!("F={}", center.nodes.f),
                format!("A={}", center.nodes.a),
            ],
        ),
        check(
            "center-edge-sets-reported",
            CheckStatus::Passed,
            vec![
                format!("E_F={}", center.edges.e_f),
                format!("P_H={}", center.edges.p_h),
                format!("P_C={}", center.edges.p_c),
                format!("visible_primary={}", center.edges.visible_primary_total()),
                format!("P_O_context={}", center.edges.p_o),
                format!("P_B_context={}", center.edges.p_b),
            ],
        ),
        check(
            "ruler-highlight-count-reported",
            CheckStatus::Passed,
            vec![format!(
                "ruler_highlights={}",
                center.marks.ruler_highlights
            )],
        ),
        check(
            "artifact-components-reported",
            CheckStatus::Passed,
            vec![
                format!("weak={}", center.components.weak),
                format!("roots={}", center.components.roots),
                format!("orphan_artifacts={}", center.components.orphan_artifacts),
                format!("weakly_connected={}", center.components.weakly_connected),
            ],
        ),
        check(
            "top-strip-present",
            missing_status(layout.top_strip_present),
            vec![format!("top_strip_present={}", layout.top_strip_present)],
        ),
        check(
            "right-inspector-present",
            missing_status(layout.right_inspector_present && inspector.right_inspector_present),
            vec![
                format!(
                    "layout.right_inspector_present={}",
                    layout.right_inspector_present
                ),
                format!(
                    "inspector.right_inspector_present={}",
                    inspector.right_inspector_present
                ),
            ],
        ),
        check(
            "bottom-timeline-present",
            missing_status(layout.bottom_timeline_present && timeline.bottom_timeline_present),
            vec![
                format!(
                    "layout.bottom_timeline_present={}",
                    layout.bottom_timeline_present
                ),
                format!(
                    "timeline.bottom_timeline_present={}",
                    timeline.bottom_timeline_present
                ),
            ],
        ),
        check(
            "selected-item-text",
            if inspector.selected_detail.is_some() {
                CheckStatus::Passed
            } else {
                CheckStatus::NotApplicable
            },
            vec![format!(
                "selected_detail_present={}",
                inspector.selected_detail.is_some()
            )],
        ),
        check(
            "unavailable-drilldowns-classified",
            missing_status(inspector.unavailable_reason_classified),
            vec![format!(
                "unavailable_reason_classified={}",
                inspector.unavailable_reason_classified
            )],
        ),
    ]
}

fn check(id: &str, status: CheckStatus, evidence: Vec<String>) -> Check {
    Check {
        id: id.to_owned(),
        status,
        evidence,
    }
}

fn bool_status(value: bool) -> CheckStatus {
    if value {
        CheckStatus::Passed
    } else {
        CheckStatus::Failed
    }
}

fn missing_status(value: bool) -> CheckStatus {
    if value {
        CheckStatus::Passed
    } else {
        CheckStatus::Missing
    }
}
