use ratatui::{
    layout::{Constraint, Direction, Layout, Rect}, style::{Color, Style, Stylize as _}, text::{Line, Span}, widgets::{Block, Paragraph}, Frame
};

use crate::ModelId;
use crate::llm::types::newtypes::ModelName;
use ploke_core::ArcStr;

#[derive(Debug)]
pub struct EmbeddingBrowserItem {
    pub id: ModelId,
    pub name: ModelName,
    pub context_length: Option<u32>,
    /// Input cost in USD per token (displayed as USD per 1M tokens).
    pub prompt_cost: Option<f64>,
    pub description: ArcStr,
    pub expanded: bool,
}

#[derive(Debug)]
pub struct EmbeddingBrowserState {
    pub visible: bool,
    pub keyword: String,
    pub items: Vec<EmbeddingBrowserItem>,
    pub selected: usize,
    pub help_visible: bool,
    // support scrolling
    pub vscroll: u16,
    pub viewport_height: u16,
}

const EMBEDDING_DETAILS_HEIGHT: usize = 3;
const EMBEDDING_BROWSER_HEADER_HEIGHT: usize = 1;

pub fn render_embedding_browser<'a>(
    frame: &mut Frame<'_>,
    eb: &EmbeddingBrowserState,
) -> (Rect, Rect, Style, Vec<Line<'a>>) {
    let area = frame.area();
    let width = area.width.saturating_mul(8) / 10;
    let height = area.height.saturating_mul(8) / 10;
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);
    let rect = ratatui::layout::Rect::new(x, y, width.max(40), height.max(10));

    // Clear the underlying content in the overlay area to avoid bleed-through
    frame.render_widget(ratatui::widgets::Clear, rect);

    // Split overlay into body + footer (help)
    let footer_height = if eb.help_visible { 6 } else { 1 };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
        .split(rect);
    let body_area = layout[0];
    let footer_area = layout[1];

    let overlay_style = Style::new().fg(Color::LightBlue);
    let mut lines: Vec<Line> = Vec::new();

    if eb.items.is_empty() {
        lines.push(Line::from(Span::styled(
            "Loading embedding models…",
            overlay_style,
        )));
    }

    let selected_style = Style::new().fg(Color::Black).bg(Color::LightCyan);
    let detail_style = Style::new().fg(Color::Blue).dim();

    for (i, it) in eb.items.iter().enumerate() {
        let mut line = Line::from(vec![
            Span::styled(
                if i == eb.selected { ">" } else { " " },
                if i == eb.selected {
                    selected_style
                } else {
                    overlay_style
                },
            ),
            Span::raw(" "),
            Span::styled(
                format!("{} — {}", it.id, it.name.as_str()),
                if i == eb.selected {
                    selected_style
                } else {
                    overlay_style
                },
            ),
        ]);
        line.style = if i == eb.selected {
            selected_style
        } else {
            overlay_style
        };
        lines.push(line);

        if it.expanded {
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
                format!(
                    "    prompt_cost: {}",
                    it.prompt_cost
                        .map(|v| format!("${:.3}", v))
                        .unwrap_or_else(|| "-".to_string())
                ),
                detail_style,
            )));
            let desc_preview: String = it.description.chars().take(120).collect();
            lines.push(Line::from(Span::styled(
                format!("    desc: {}", desc_preview),
                detail_style,
            )));
        }
    }

    (body_area, footer_area, overlay_style, lines)
}

pub fn embedding_browser_detail_lines(it: &EmbeddingBrowserItem) -> usize {
    if !it.expanded {
        return 0;
    }
    EMBEDDING_DETAILS_HEIGHT
}

pub fn embedding_browser_total_lines(eb: &EmbeddingBrowserState) -> usize {
    let base = EMBEDDING_BROWSER_HEADER_HEIGHT
        + eb
            .items
            .iter()
            .map(embedding_browser_detail_lines)
            .map(|it| it + 1)
            .sum::<usize>();
    if eb.items.is_empty() {
        base + 1
    } else {
        base
    }
}

pub fn embedding_browser_focus_line(eb: &EmbeddingBrowserState) -> usize {
    let header = EMBEDDING_BROWSER_HEADER_HEIGHT;
    if eb.items.is_empty() {
        return header;
    }

    let sel_idx = eb.selected.min(eb.items.len().saturating_sub(1));
    let mut line = header;
    for j in 0..sel_idx {
        let it = &eb.items[j];
        line += 1;
        line += embedding_browser_detail_lines(it);
    }
    line
}

pub(crate) fn compute_embedding_browser_scroll(body_area: Rect, eb: &mut EmbeddingBrowserState) {
    eb.viewport_height = body_area.height.saturating_sub(2);

    let total = embedding_browser_total_lines(eb);
    let focus = embedding_browser_focus_line(eb);
    let vh = eb.viewport_height as usize;
    let max_v = total.saturating_sub(vh);

    eb.vscroll = (eb.vscroll as usize).min(max_v) as u16;

    let v = eb.vscroll as usize;
    if focus < v {
        eb.vscroll = focus as u16;
    } else if focus >= v + vh {
        eb.vscroll = (focus + 1).saturating_sub(vh) as u16;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn detail_lines_zero_when_not_expanded() {
        let item = EmbeddingBrowserItem {
            id: ModelId::from_str("author/model").expect("model id"),
            name: ModelName::new("model"),
            context_length: None,
            prompt_cost: None,
            description: ArcStr::from("desc"),
            expanded: false,
        };
        assert_eq!(embedding_browser_detail_lines(&item), 0);
    }

    #[test]
    fn total_lines_accounts_for_loading() {
        let eb = EmbeddingBrowserState {
            visible: true,
            keyword: "kw".to_string(),
            items: Vec::new(),
            selected: 0,
            help_visible: false,
            vscroll: 0,
            viewport_height: 10,
        };
        assert_eq!(embedding_browser_total_lines(&eb), EMBEDDING_BROWSER_HEADER_HEIGHT + 1);
    }
}
