use eframe::egui::{self, Color32, Response, RichText, ScrollArea, Ui, Widget};
use ploke_eval::walk_client::{PhaseInfo, PhaseInventory, WalkPhase, WalkPosition};

pub(in crate::app) struct PhaseRail<'a> {
    pub(in crate::app) phases: &'a PhaseInventory,
    pub(in crate::app) position: Option<&'a WalkPosition>,
    pub(in crate::app) current: Option<WalkPhase>,
}

impl PhaseRail<'_> {
    pub(in crate::app) fn show(self, ui: &mut Ui) {
        ui.heading("Phases");
        ui.add_space(6.0);
        ScrollArea::vertical().show(ui, |ui| {
            for phase in &self.phases.phases {
                ui.add(PhaseRow {
                    phase,
                    position: self.position,
                    current: self.current,
                });
            }
        });
    }
}

struct PhaseRow<'a> {
    phase: &'a PhaseInfo,
    position: Option<&'a WalkPosition>,
    current: Option<WalkPhase>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowState {
    Idle,
    Current,
    NoSession,
    Reconstruction,
    Unpositioned,
    Session,
    Legacy,
}

impl RowState {
    const fn badge(self) -> Option<&'static str> {
        match self {
            Self::Idle | Self::Current => None,
            Self::NoSession => Some("NO SESSION"),
            Self::Reconstruction => Some("RECONSTRUCTED"),
            Self::Unpositioned => Some("NO CURSOR"),
            Self::Session => Some("SESSION"),
            Self::Legacy => Some("LEGACY"),
        }
    }

    const fn hint(self) -> Option<&'static str> {
        match self {
            Self::Idle => None,
            Self::Current => Some("Latest response phase; status authority has not been observed."),
            Self::NoSession => Some("No durable controller session exists for this run."),
            Self::Reconstruction => Some(
                "Phase reconstructed from pre-session evidence; no durable session cursor exists yet.",
            ),
            Self::Unpositioned => {
                Some("A durable controller session exists, but it has no committed phase cursor.")
            }
            Self::Session => Some("Phase proven by the durable controller-session cursor."),
            Self::Legacy => Some(
                "Phase decoded from an older protocol that did not identify its authority source.",
            ),
        }
    }

    const fn colors(self) -> (Color32, Color32) {
        match self {
            Self::Idle => (Color32::from_rgb(31, 31, 34), Color32::from_rgb(62, 62, 66)),
            Self::Current | Self::Session => (
                Color32::from_rgb(35, 74, 92),
                Color32::from_rgb(113, 180, 166),
            ),
            Self::NoSession => (
                Color32::from_rgb(45, 48, 53),
                Color32::from_rgb(132, 139, 148),
            ),
            Self::Reconstruction => (
                Color32::from_rgb(76, 58, 26),
                Color32::from_rgb(230, 177, 76),
            ),
            Self::Unpositioned => (
                Color32::from_rgb(78, 42, 35),
                Color32::from_rgb(229, 145, 112),
            ),
            Self::Legacy => (
                Color32::from_rgb(55, 48, 65),
                Color32::from_rgb(198, 171, 92),
            ),
        }
    }
}

fn row_state(
    phase: WalkPhase,
    position: Option<&WalkPosition>,
    current: Option<WalkPhase>,
) -> RowState {
    let Some(position) = position else {
        return if current == Some(phase) {
            RowState::Current
        } else {
            RowState::Idle
        };
    };
    if position.phase() != phase {
        return RowState::Idle;
    }
    match position {
        WalkPosition::NoSession => RowState::NoSession,
        WalkPosition::Reconstruction { .. } => RowState::Reconstruction,
        WalkPosition::Unpositioned { .. } => RowState::Unpositioned,
        WalkPosition::Session { .. } => RowState::Session,
        WalkPosition::Legacy { .. } => RowState::Legacy,
    }
}

impl Widget for PhaseRow<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let state = row_state(self.phase.phase, self.position, self.current);
        let (fill, stroke) = state.colors();
        let mut response = egui::Frame::new()
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, stroke))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.phase.id).monospace().strong());
                    ui.label(&self.phase.label);
                });
                if let Some(badge) = state.badge() {
                    let badge = ui.label(RichText::new(badge).small().strong().color(stroke));
                    if let Some(hint) = state.hint() {
                        badge.on_hover_text(hint);
                    }
                }
                for next in &self.phase.next {
                    ui.label(
                        RichText::new(format!("-> {} ({})", next.phase_id, next.edge))
                            .small()
                            .color(Color32::LIGHT_GRAY),
                    );
                }
            })
            .response;
        if let Some(hint) = state.hint() {
            response = response.on_hover_text(hint);
        }
        ui.add_space(5.0);
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(value: serde_json::Value) -> WalkPosition {
        serde_json::from_value(value).expect("valid test position")
    }

    #[test]
    fn authority_sources_have_distinct_current_row_states() {
        let reconstruction = WalkPosition::Reconstruction {
            phase: WalkPhase::R4c,
        };
        let session = position(serde_json::json!({
            "source": "session",
            "version": {
                "session_id": "00000000-0000-0000-0000-000000000001",
                "cursor": {
                    "phase": "r4c",
                    "evidence": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                },
                "journal_revision": 1
            }
        }));
        let legacy = position(serde_json::json!({
            "source": "legacy",
            "phase": "r4c",
            "version": {
                "session_id": null,
                "cursor": null,
                "journal_revision": 0
            }
        }));
        let unpositioned = position(serde_json::json!({
            "source": "unpositioned",
            "version": {
                "session_id": "00000000-0000-0000-0000-000000000002",
                "cursor": null,
                "journal_revision": 1
            }
        }));

        assert_eq!(
            row_state(WalkPhase::R4c, Some(&reconstruction), None),
            RowState::Reconstruction
        );
        assert_eq!(
            row_state(WalkPhase::R4c, Some(&session), None),
            RowState::Session
        );
        assert_eq!(
            row_state(WalkPhase::R4c, Some(&legacy), None),
            RowState::Legacy
        );
        assert_eq!(
            row_state(WalkPhase::Empty, Some(&unpositioned), None),
            RowState::Unpositioned
        );
        assert_eq!(
            row_state(WalkPhase::R4a, Some(&reconstruction), None),
            RowState::Idle
        );
    }

    #[test]
    fn no_session_and_phase_fallback_remain_distinct() {
        assert_eq!(
            row_state(WalkPhase::Empty, Some(&WalkPosition::NoSession), None),
            RowState::NoSession
        );
        assert_eq!(
            row_state(WalkPhase::R3, Some(&WalkPosition::NoSession), None),
            RowState::Idle
        );
        assert_eq!(
            row_state(WalkPhase::R6, None, Some(WalkPhase::R6)),
            RowState::Current
        );
        assert_eq!(row_state(WalkPhase::R6, None, None), RowState::Idle);
    }

    #[test]
    fn authority_badge_remains_queryable_with_a_long_phase_label() {
        use egui_kittest::{Harness, kittest::Queryable};

        let phase = PhaseInfo {
            phase: WalkPhase::R11a,
            id: "R11a".to_string(),
            label: "A deliberately long phase label that consumes the first row".to_string(),
            typestate: "R11a".to_string(),
            next: Vec::new(),
        };
        let position = WalkPosition::Reconstruction {
            phase: WalkPhase::R11a,
        };
        let harness = Harness::builder()
            .with_size(egui::Vec2::new(280.0, 140.0))
            .build_ui(move |ui| {
                ui.add(PhaseRow {
                    phase: &phase,
                    position: Some(&position),
                    current: Some(WalkPhase::R11a),
                });
            });

        let badges: Vec<_> = harness.get_all_by_label("RECONSTRUCTED").collect();
        assert_eq!(badges.len(), 1);
        assert!(
            badges[0].rect().max.x <= 280.0,
            "authority badge must remain inside the default-width phase rail"
        );
    }

    #[test]
    fn unpositioned_badge_is_textually_queryable() {
        use egui_kittest::{Harness, kittest::Queryable};

        let phase = PhaseInfo {
            phase: WalkPhase::Empty,
            id: "Empty".to_string(),
            label: "No committed controller cursor".to_string(),
            typestate: "Empty".to_string(),
            next: Vec::new(),
        };
        let position = position(serde_json::json!({
            "source": "unpositioned",
            "version": {
                "session_id": "00000000-0000-0000-0000-000000000003",
                "cursor": null,
                "journal_revision": 2
            }
        }));
        let harness = Harness::builder()
            .with_size(egui::Vec2::new(280.0, 140.0))
            .build_ui(move |ui| {
                ui.add(PhaseRow {
                    phase: &phase,
                    position: Some(&position),
                    current: Some(WalkPhase::Empty),
                });
            });

        assert_eq!(harness.get_all_by_label("NO CURSOR").count(), 1);
    }
}
