use eframe::egui::Color32;
use ploke_records::branch::TreatmentBranchStatus;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewStyle {
    pub layout: LayoutStyle,
    pub labels: LabelStyle,
    pub edge: EdgeStyle,
}

impl Default for ViewStyle {
    fn default() -> Self {
        Self {
            layout: LayoutStyle::default(),
            labels: LabelStyle::default(),
            edge: EdgeStyle::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutStyle {
    pub row_distance: f32,
    pub column_distance: f32,
    pub fit_padding: f32,
    pub node_radius: f32,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            row_distance: 140.0,
            column_distance: 170.0,
            fit_padding: 0.18,
            node_radius: 11.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelStyle {
    pub artifact_id_prefix: &'static str,
    pub patch_id_prefix: &'static str,
}

impl LabelStyle {
    pub fn artifact(self, id: &str) -> String {
        self.trim_artifact(id).to_owned()
    }

    fn trim_artifact(self, id: &str) -> &str {
        id.strip_prefix(self.artifact_id_prefix).unwrap_or(id)
    }
}

impl Default for LabelStyle {
    fn default() -> Self {
        Self {
            artifact_id_prefix: "artifact:",
            patch_id_prefix: "patch:",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeStyle {
    pub curve: CurveStyle,
    pub label: EdgeLabelStyle,
    pub colors: StatusColors,
    pub normal_width: f32,
    pub selected_width: f32,
    pub hit_tolerance: f32,
}

impl Default for EdgeStyle {
    fn default() -> Self {
        Self {
            curve: CurveStyle::default(),
            label: EdgeLabelStyle::default(),
            colors: StatusColors::default(),
            normal_width: 2.0,
            selected_width: 4.0,
            hit_tolerance: 8.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurveStyle {
    pub rank_handle_fraction: f32,
    pub min_handle: f32,
    pub endpoint_max_arc_fraction: f32,
    pub hit_segments: usize,
}

impl Default for CurveStyle {
    fn default() -> Self {
        Self {
            rank_handle_fraction: 0.5,
            min_handle: 48.0,
            endpoint_max_arc_fraction: 0.35,
            hit_segments: 16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeLabelStyle {
    pub font_size: f32,
    pub gap: f32,
}

impl Default for EdgeLabelStyle {
    fn default() -> Self {
        Self {
            font_size: 13.0,
            gap: 4.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusColors {
    pub synthesized: Color32,
    pub selected: Color32,
    pub applied: Color32,
    pub restored: Color32,
    pub dropped: Color32,
}

impl StatusColors {
    pub fn color(self, status: TreatmentBranchStatus) -> Color32 {
        match status {
            TreatmentBranchStatus::Synthesized => self.synthesized,
            TreatmentBranchStatus::Selected => self.selected,
            TreatmentBranchStatus::Applied => self.applied,
            TreatmentBranchStatus::Restored => self.restored,
            TreatmentBranchStatus::Dropped => self.dropped,
        }
    }
}

impl Default for StatusColors {
    fn default() -> Self {
        Self {
            synthesized: Color32::from_rgb(118, 128, 142),
            selected: Color32::from_rgb(52, 145, 95),
            applied: Color32::from_rgb(55, 118, 184),
            restored: Color32::from_rgb(126, 116, 95),
            dropped: Color32::from_rgb(178, 72, 72),
        }
    }
}
