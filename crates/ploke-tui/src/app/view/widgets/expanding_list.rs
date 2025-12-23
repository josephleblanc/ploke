use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
};

use crate::app::view::components::model_browser::ModelBrowserState;

pub struct ExpandingList<'a> {
    rect: Rect,
    style: Style,
    lines: Vec<Line<'a>>,
}

pub fn render_model_browser<'a>(
    frame: &mut Frame<'_>,
    mb: &ModelBrowserState,
) -> (Rect, Rect, Style, Vec<Line<'a>>) {
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

    // Build list content (styled). Header moved to Block title; keep only list lines here.
    let mut lines: Vec<Line> = Vec::new();

    // Loading indicator when opened before results arrive
    if mb.items.is_empty() {
        lines.push(Line::from(Span::styled("Loading models…", overlay_style)));
    }

    // Selected row highlighting
    let selected_style = Style::new().fg(Color::Black).bg(Color::LightCyan);
    let detail_style = Style::new().fg(Color::Blue).dim();

    for (i, it) in mb.items.iter().enumerate() {
        let title = if let Some(name) = &it.name {
            if name.as_str().is_empty() {
                it.id.to_string()
            } else {
                format!("{} — {}", it.id, name.as_str())
            }
        } else {
            it.id.to_string()
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
                        .map(|v| format!("${:.3}", v))
                        .unwrap_or_else(|| "-".to_string()),
                    it.output_cost
                        .map(|v| format!("${:.3}", v))
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
                for (row_idx, p) in it.providers.iter().enumerate() {
                    let indent = "      ";
                    let is_sel = mb.provider_select_active
                        && i == mb.selected
                        && row_idx == mb.provider_selected;
                    let row_style = if is_sel { selected_style } else { detail_style };
                    let pointer = if is_sel { ">" } else { "-" };
                    lines.push(Line::from(Span::styled(
                        format!(
                            "{indent}{} {} in=${} out=${} ctx={} tools={}",
                            pointer,
                            p.provider_name,
                            format_args!("{:.3}", p.input_cost),
                            format_args!("{:.3}", p.output_cost),
                            format_args!("{:.0}", p.context_length),
                            p.supports_tools,
                        ),
                        row_style,
                    )));
                }
            }
        }
    }
    (body_area, footer_area, overlay_style, lines)
}
