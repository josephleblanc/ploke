use std::sync::Arc;

use crossterm::event::KeyEvent;
use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::Frame;
use ratatui::widgets::{Block, Paragraph, Wrap};

use crate::app::input;
use crate::app::overlay::{OverlayAction, OverlayKind};
use crate::app::view::components::approvals::{ApprovalsState, render_approvals_overlay};
use crate::app::view::components::config_overlay::{
    ConfigOverlayState, render_config_overlay,
};
use crate::app::view::components::context_browser::{ContextSearchState, render_context_search};
use crate::app::view::components::embedding_browser::{
    EmbeddingBrowserState, compute_embedding_browser_scroll, render_embedding_browser,
};
use crate::app::view::components::model_browser::{
    ModelBrowserState, compute_browser_scroll, render_model_browser,
};
use crate::app_state::AppState;

#[derive(Debug)]
pub enum ActiveOverlay {
    Config(ConfigOverlayState),
    ModelBrowser(ModelBrowserState),
    EmbeddingBrowser(EmbeddingBrowserState),
    ContextBrowser(ContextSearchState),
    Approvals(ApprovalsState),
}

#[derive(Debug, Default)]
pub struct OverlayManager {
    active: Option<ActiveOverlay>,
}

impl OverlayManager {
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn is_config_open(&self) -> bool {
        matches!(self.active, Some(ActiveOverlay::Config(_)))
    }

    pub fn is_approvals_open(&self) -> bool {
        matches!(self.active, Some(ActiveOverlay::Approvals(_)))
    }

    pub fn open_config(&mut self, state: ConfigOverlayState) {
        self.active = Some(ActiveOverlay::Config(state));
    }

    pub fn open_model_browser(&mut self, state: ModelBrowserState) {
        self.active = Some(ActiveOverlay::ModelBrowser(state));
    }

    pub fn open_embedding_browser(&mut self, state: EmbeddingBrowserState) {
        self.active = Some(ActiveOverlay::EmbeddingBrowser(state));
    }

    pub fn open_context_browser(&mut self, state: ContextSearchState) {
        self.active = Some(ActiveOverlay::ContextBrowser(state));
    }

    pub fn open_approvals(&mut self, state: ApprovalsState) {
        self.active = Some(ActiveOverlay::Approvals(state));
    }

    pub fn close_active(&mut self) {
        self.active = None;
    }

    pub fn close_kind(&mut self, kind: OverlayKind) {
        if self.active_kind() == Some(kind) {
            self.active = None;
        }
    }

    pub fn context_state(&self) -> Option<&ContextSearchState> {
        match self.active.as_ref() {
            Some(ActiveOverlay::ContextBrowser(state)) => Some(state),
            _ => None,
        }
    }

    pub fn context_state_mut(&mut self) -> Option<&mut ContextSearchState> {
        match self.active.as_mut() {
            Some(ActiveOverlay::ContextBrowser(state)) => Some(state),
            _ => None,
        }
    }

    pub fn model_browser_state(&self) -> Option<&ModelBrowserState> {
        match self.active.as_ref() {
            Some(ActiveOverlay::ModelBrowser(state)) => Some(state),
            _ => None,
        }
    }

    pub fn model_browser_state_mut(&mut self) -> Option<&mut ModelBrowserState> {
        match self.active.as_mut() {
            Some(ActiveOverlay::ModelBrowser(state)) => Some(state),
            _ => None,
        }
    }

    pub fn embedding_browser_state(&self) -> Option<&EmbeddingBrowserState> {
        match self.active.as_ref() {
            Some(ActiveOverlay::EmbeddingBrowser(state)) => Some(state),
            _ => None,
        }
    }

    pub fn embedding_browser_state_mut(&mut self) -> Option<&mut EmbeddingBrowserState> {
        match self.active.as_mut() {
            Some(ActiveOverlay::EmbeddingBrowser(state)) => Some(state),
            _ => None,
        }
    }

    pub fn approvals_state(&self) -> Option<&ApprovalsState> {
        match self.active.as_ref() {
            Some(ActiveOverlay::Approvals(state)) => Some(state),
            _ => None,
        }
    }

    pub fn handle_input(&mut self, key: KeyEvent) -> Vec<OverlayAction> {
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };

        let mut actions = match active {
            ActiveOverlay::Config(state) => {
                if input::config_overlay::handle_config_overlay_input(state, key) {
                    self.active = None;
                }
                Vec::new()
            }
            ActiveOverlay::ModelBrowser(state) => {
                input::model_browser::handle_model_browser_input(state, key)
            }
            ActiveOverlay::EmbeddingBrowser(state) => {
                input::embedding_browser::handle_embedding_browser_input(state, key)
            }
            ActiveOverlay::ContextBrowser(state) => {
                input::context_browser::handle_context_browser_input(state, key)
            }
            ActiveOverlay::Approvals(state) => {
                input::approvals::handle_approvals_input(state, key)
            }
        };

        if actions.is_empty() {
            return actions;
        }

        // Filter out close actions for non-active overlays.
        actions.retain(|action| match action {
            OverlayAction::CloseOverlay(kind) => self.active_kind() == Some(*kind),
            _ => true,
        });

        actions
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, state: &Arc<AppState>) {
        if let Some(active) = self.active.as_mut() {
            match active {
                ActiveOverlay::Config(overlay) => {
                    render_config_overlay(frame, overlay);
                }
                ActiveOverlay::ModelBrowser(overlay) => {
                    Self::render_model_browser(frame, overlay);
                }
                ActiveOverlay::EmbeddingBrowser(overlay) => {
                    Self::render_embedding_browser(frame, overlay);
                }
                ActiveOverlay::ContextBrowser(overlay) => {
                    Self::render_context_browser(frame, overlay);
                }
                ActiveOverlay::Approvals(overlay) => {
                    Self::render_approvals(frame, state, overlay);
                }
            }
        }
    }

    fn active_kind(&self) -> Option<OverlayKind> {
        match self.active.as_ref() {
            Some(ActiveOverlay::Approvals(_)) => Some(OverlayKind::Approvals),
            Some(ActiveOverlay::ContextBrowser(_)) => Some(OverlayKind::ContextBrowser),
            Some(ActiveOverlay::EmbeddingBrowser(_)) => Some(OverlayKind::EmbeddingBrowser),
            Some(ActiveOverlay::ModelBrowser(_)) => Some(OverlayKind::ModelBrowser),
            Some(ActiveOverlay::Config(_)) => None,
            None => None,
        }
    }

    fn render_model_browser(frame: &mut Frame<'_>, overlay: &mut ModelBrowserState) {
        let (body_area, footer_area, overlay_style, lines) = render_model_browser(frame, overlay);

        // Keep focused row visible and clamp vscroll
        compute_browser_scroll(body_area, overlay);

        let widget = Paragraph::new(lines)
            .style(overlay_style)
            .block(
                Block::bordered()
                    .title(format!(
                        " Model Browser — {} results for \"{}\" ",
                        overlay.items.len(),
                        overlay.keyword
                    ))
                    .style(overlay_style),
            )
            // Preserve leading indentation in detail lines
            .wrap(Wrap { trim: false })
            .scroll((overlay.vscroll, 0));
        frame.render_widget(widget, body_area);

        // Footer: bottom-right help toggle or expanded help
        if overlay.help_visible {
            let help = Paragraph::new(
                "Keys: s=select  Enter/Space=toggle details  j/k,↑/↓=navigate  q/Esc=close\n\
                 Save/Load/Search:\n\
                 - model save [path] [--with-keys]\n\
                 - model load [path]\n\
                 - model search <keyword>",
            )
            .style(overlay_style)
            .block(Block::bordered().title(" Help ").style(overlay_style))
            .wrap(Wrap { trim: true });
            frame.render_widget(help, footer_area);
        } else {
            let hint = Paragraph::new(" ? Help ")
                .style(overlay_style)
                .alignment(Alignment::Right)
                .block(Block::default().style(overlay_style));
            frame.render_widget(hint, footer_area);
        }
    }

    fn render_embedding_browser(frame: &mut Frame<'_>, overlay: &mut EmbeddingBrowserState) {
        let (body_area, footer_area, overlay_style, lines) =
            render_embedding_browser(frame, overlay);

        compute_embedding_browser_scroll(body_area, overlay);

        let widget = Paragraph::new(lines)
            .style(overlay_style)
            .block(
                Block::bordered()
                    .title(format!(
                        " Embedding Models — {} results for \"{}\" ",
                        overlay.items.len(),
                        overlay.keyword
                    ))
                    .style(overlay_style),
            )
            .wrap(Wrap { trim: false })
            .scroll((overlay.vscroll, 0));
        frame.render_widget(widget, body_area);

        if overlay.help_visible {
            let help = Paragraph::new(
                "Keys: s=select  Enter/Space=toggle details  j/k,↑/↓=navigate  q/Esc=close\n\
                 Command:\n\
                 - embedding search <keyword>",
            )
            .style(overlay_style)
            .block(Block::bordered().title(" Help ").style(overlay_style))
            .wrap(Wrap { trim: true });
            frame.render_widget(help, footer_area);
        } else {
            let hint = Paragraph::new(" ? Help ")
                .style(overlay_style)
                .alignment(Alignment::Right)
                .block(Block::default().style(overlay_style));
            frame.render_widget(hint, footer_area);
        }
    }

    fn render_context_browser(frame: &mut Frame<'_>, overlay: &mut ContextSearchState) {
        let (body_area, footer_area, overlay_style, lines) =
            render_context_search(frame, overlay);

        let free_width = body_area.width.saturating_sub(43) as usize;
        let trunc_search_string: String =
            overlay.input.as_str().chars().take(free_width).collect();

        // WARNING: temporarily taking this line out due to borrowing issues, need to turn it
        // on for better functionality later
        // user_search::compute_browser_scroll(body_area, overlay);

        // subtract 43 for the length of the surrounding text in the `format!` call for the
        // widget title below.
        let widget = Paragraph::new(lines)
            .style(overlay_style)
            .block(
                Block::bordered()
                    .title(format!(
                        " Context Browser — {} results for \"{}\" ",
                        overlay.items.len(),
                        trunc_search_string
                    ))
                    .style(overlay_style),
            )
            // Preserve leading indentation in detail lines
            .wrap(Wrap { trim: false })
            .scroll((overlay.vscroll, 0));
        frame.render_widget(widget, body_area);
        if overlay.help_visible {
            // NOTE: placeholder for now, not actually functional
            let help = Paragraph::new(
                "Keys: Enter/Space=toggle details  j/k,↑/↓=navigate  q/Esc=close\n\
                 Save/Load/Search:\n\
                 - model save [path] [--with-keys]\n\
                 - model load [path]\n\
                 - model search <keyword>",
            )
            .style(overlay_style)
            .block(Block::bordered().title(" Help ").style(overlay_style))
            .wrap(Wrap { trim: true });
            frame.render_widget(help, footer_area);
        } else {
            let hint = Paragraph::new(" ? Help ")
                .style(overlay_style)
                .alignment(Alignment::Right)
                .block(Block::default().style(overlay_style));
            frame.render_widget(hint, footer_area);
        }
    }

    fn render_approvals(
        frame: &mut Frame<'_>,
        state: &Arc<AppState>,
        overlay: &ApprovalsState,
    ) {
        let w = frame.area().width.saturating_mul(8) / 10;
        let h = frame.area().height.saturating_mul(8) / 10;
        let x = frame.area().x + (frame.area().width.saturating_sub(w)) / 2;
        let y = frame.area().y + (frame.area().height.saturating_sub(h)) / 2;
        let overlay_area = Rect::new(x, y, w, h);
        let _ = render_approvals_overlay(frame, overlay_area, state, overlay);
    }
}
