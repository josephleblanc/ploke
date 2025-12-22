use std::time::Duration;

use crossterm::event::KeyEvent;
use ploke_core::ArcStr;
use ratatui::Frame;

use crate::app::input;
use crate::app::view::components::approvals::{ApprovalsState, render_approvals_overlay};

use crate::ModelId;
use crate::app_state::AppState;
use crate::llm::ProviderKey;

#[derive(Debug, Clone)]
pub enum OverlayAction {
    CloseOverlay(OverlayKind),
    RequestModelEndpoints { model_id: ModelId },
    SelectModel {
        model_id: ModelId,
        provider: Option<ProviderKey>,
    },
    SelectEmbeddingModel {
        model_id: ModelId,
        provider: Option<ArcStr>,
    },
    ApproveSelectedProposal,
    DenySelectedProposal,
    OpenSelectedProposalInEditor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Approvals,
    ContextBrowser,
    EmbeddingBrowser,
    ModelBrowser,
}

/// Overlay behavior contract used by the overlay manager.
///
/// Implementations own local UI state and translate raw input (`KeyEvent`) into
/// intent-style commands (`OverlayAction`) for the App to handle. This keeps side
/// effects (IO, network, persistence) centralized and testable.
///
/// `handle_input` returns a `Vec<OverlayAction>` to support multi-intent inputs
/// (for example, a key that both selects an item and closes the overlay), while
/// still allowing most inputs to emit zero or one action.
///
/// Lifecycle hooks (`on_open`/`on_close`) are required so overlays can explicitly
/// reset transient state or perform cleanup when the overlay is shown/hidden.
///
/// `render` should be pure: no IO, no async work, and no mutation outside the
/// overlay. Overlays may read `AppState` to build a view model, but should avoid
/// heavy allocations inside render paths.
///
/// `tick` receives a `Duration` so overlays can drive time-based behavior (spinner
/// frames, debounced updates, timeouts). The cadence is controlled by the App and
/// may be dynamic rather than a fixed interval.
pub trait Overlay {
    fn on_open(&mut self);
    fn on_close(&mut self);
    fn handle_input(&mut self, key: KeyEvent) -> Vec<OverlayAction>;
    fn render(&mut self, frame: &mut Frame<'_>, state: &std::sync::Arc<AppState>);
    fn tick(&mut self, dt: Duration);
}

/// Render an overlay using a lightweight generic wrapper to keep the public API ergonomic.
pub fn render_overlay<O: Overlay>(
    overlay: &mut O,
    frame: &mut Frame<'_>,
    state: &std::sync::Arc<AppState>,
) {
    render_overlay_impl(overlay, frame, state);
}

/// Heavy render path uses dynamic dispatch to avoid monomorphization bloat.
fn render_overlay_impl(
    overlay: &mut dyn Overlay,
    frame: &mut Frame<'_>,
    state: &std::sync::Arc<AppState>,
) {
    overlay.render(frame, state);
}

/// Handle input via a lightweight generic wrapper for callers.
pub fn handle_overlay_input<O: Overlay>(overlay: &mut O, key: KeyEvent) -> Vec<OverlayAction> {
    handle_overlay_input_impl(overlay, key)
}

/// Heavy input path uses dynamic dispatch to avoid monomorphization bloat.
fn handle_overlay_input_impl(overlay: &mut dyn Overlay, key: KeyEvent) -> Vec<OverlayAction> {
    overlay.handle_input(key)
}

/// Tick via a lightweight generic wrapper for callers.
pub fn tick_overlay<O: Overlay>(overlay: &mut O, dt: Duration) {
    tick_overlay_impl(overlay, dt)
}

/// Heavy tick path uses dynamic dispatch to avoid monomorphization bloat.
fn tick_overlay_impl(overlay: &mut dyn Overlay, dt: Duration) {
    overlay.tick(dt);
}

impl Overlay for ApprovalsState {
    fn on_open(&mut self) {}

    fn on_close(&mut self) {}

    fn handle_input(&mut self, key: KeyEvent) -> Vec<OverlayAction> {
        input::approvals::handle_approvals_input(self, key)
    }

    fn render(&mut self, frame: &mut Frame<'_>, state: &std::sync::Arc<AppState>) {
        let w = frame.area().width.saturating_mul(8) / 10;
        let h = frame.area().height.saturating_mul(8) / 10;
        let x = frame.area().x + (frame.area().width.saturating_sub(w)) / 2;
        let y = frame.area().y + (frame.area().height.saturating_sub(h)) / 2;
        let overlay_area = ratatui::layout::Rect::new(x, y, w, h);
        let _ = render_approvals_overlay(frame, overlay_area, state, self);
    }

    fn tick(&mut self, _dt: Duration) {}
}
