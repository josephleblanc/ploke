use ploke_core::ArcStr;
use ratatui::Frame;
use ratatui::layout::{ Rect, Layout, Direction, Constraint };
use ratatui::style::{ Color, Style, Stylize as _ };

use crate::app::input::keymap::{Action, to_action};
use crate::app::types::{Mode, RenderMsg};
use crate::app::utils::truncate_uuid;
use crate::app::view::components::conversation::ConversationView;
use crate::app::view::components::input_box::InputView;
use crate::llm2::{ModelId, ModelName};
use crate::user_config::OPENROUTER_URL;
use color_eyre::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton,
    MouseEvent, MouseEventKind,
};
use crossterm::execute;
use itertools::Itertools;
// use message_item::{measure_messages, render_messages}; // now handled by ConversationView
use ploke_db::search_similar;
use ratatui::text::{Line, Span};
use ratatui::widgets::Gauge;
// use textwrap::wrap; // moved into InputView
use tokio::sync::oneshot;
use toml::to_string;
use tracing::instrument;

#[derive(Debug)]
pub struct ModelBrowserItem {
    pub id: ModelId,
    pub name: Option<ModelName>,
    pub context_length: Option<u32>,
    pub input_cost: Option<f64>,
    pub output_cost: Option<f64>,
    pub supports_tools: bool,
    pub providers: Vec<ModelProviderRow>,
    pub expanded: bool,
    // Runtime flags for async provider loading and deferred selection
    pub loading_providers: bool,
    pub pending_select: bool,
}

#[derive(Debug)]
pub struct ModelProviderRow {
    pub name: ArcStr,
    pub context_length: u32,
    pub input_cost: f64,
    pub output_cost: f64,
    pub supports_tools: bool,
}

#[derive(Debug)]
pub struct ModelBrowserState {
    pub visible: bool,
    pub keyword: String,
    pub items: Vec<ModelBrowserItem>,
    pub selected: usize,
    // Toggle for bottom-right help panel within the Model Browser overlay
    pub help_visible: bool,
}


pub fn render_model_browser<'a>(frame: &mut Frame<'_>, mb: &ModelBrowserState) -> (Rect, Rect, Style, Vec<Line<'a>>) {
    let area = frame.area();
    let width = area.width.saturating_mul(8) / 10;
    let height = area.height.saturating_mul(8) / 10;
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);
    let rect = ratatui::layout::Rect::new(x, y, width.max(40), height.max(10));

    // Clear the underlying content in the overlay area to avoid "bleed-through"
    frame.render_widget(ratatui::widgets::Clear, rect);

    // Split overlay into body + footer (help)
    let footer_height = if mb.help_visible { 6 } else { 1 };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
        .split(rect);
    let body_area = layout[0];
    let footer_area = layout[1];

    // Consistent overlay style (foreground/background)
    // Choose a high-contrast, uniform scheme that doesn't depend on background UI
    let overlay_style = Style::new().fg(Color::LightBlue);

    // Build list content (styled)
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(
            "Model Browser — {} results for \"{}\"",
            mb.items.len(),
            mb.keyword
        ),
        overlay_style,
    )));
    lines.push(Line::from(Span::styled(
        "Instructions: ↑/↓ or j/k to navigate, Enter/Space to expand, s to select, ? to toggle help, q/Esc to close.",
        overlay_style,
    )));
    lines.push(Line::from(Span::raw("")));

    // Loading indicator when opened before results arrive
    if mb.items.is_empty() {
        lines.push(Line::from(Span::styled("Loading models…", overlay_style)));
    }

    // Selected row highlighting
    let selected_style = Style::new().fg(Color::Black).bg(Color::LightCyan);
    let detail_style = Style::new().fg(Color::Blue).dim();

    for (i, it) in mb.items.iter().enumerate() {
        let title = if let Some(name) = &it.name {
            if name.is_empty() {
                it.id.clone()
            } else {
                format!("{} — {}", it.id, name)
            }
        } else {
            it.id.clone()
        };

        let mut line = Line::from(vec![
            Span::styled(
                if i == mb.selected { ">" } else { " " },
                if i == mb.selected {
                    selected_style
                } else {
                    overlay_style
                },
            ),
            Span::raw(" "),
            Span::styled(
                title,
                if i == mb.selected {
                    selected_style
                } else {
                    overlay_style
                },
            ),
        ]);
        // Ensure entire line style is applied (for background fill)
        line.style = if i == mb.selected {
            selected_style
        } else {
            overlay_style
        };
        lines.push(line);

        if it.expanded {
            // Indented details for readability while navigating (preserve spaces; do not trim)
            lines.push(Line::from(Span::styled(
                format!(
                    "    context_length: {}",
                    it.context_length
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string())
                ),
                detail_style,
            )));
            lines.push(Line::from(Span::styled(
                format!("    supports_tools: {}", it.supports_tools),
                detail_style,
            )));
            lines.push(Line::from(Span::styled(
                format!(
                    "    pricing: in={} out={}",
                    it.input_cost
                        .map(|v| format!("{:.6}", v))
                        .unwrap_or_else(|| "-".to_string()),
                    it.output_cost
                        .map(|v| format!("{:.6}", v))
                        .unwrap_or_else(|| "-".to_string())
                ),
                detail_style,
            )));

            // Provider breakdown (with loading/empty states)
            lines.push(Line::from(Span::styled("    providers:", detail_style)));
            if it.loading_providers {
                lines.push(Line::from(Span::styled("      (loading…)", detail_style)));
            } else if it.providers.is_empty() {
                lines.push(Line::from(Span::styled("      (none)", detail_style)));
            } else {
                for p in &it.providers {
                    lines.push(Line::from(Span::styled(
                        format!(
                            "      - {}  tools={}  ctx={}  pricing: in={} out={}",
                            p.name.as_ref(),
                            p.supports_tools,
                            format_args!("{:.0}", p.context_length),
                            format_args!("{:.3}", p.input_cost * 1_000_000.0),
                            format_args!("{:.3}", p.output_cost * 1_000_000.0                                             ),
                        ),
                        detail_style,
                    )));
                }
            }
        }
    }
    (body_area, footer_area, overlay_style, lines)
}
