use std::rc::Rc;

use super::*;
// In app.rs, replace the List rendering with custom Paragraph-based rendering

#[derive(Debug, Clone)]
struct MessageItem {
    id: Uuid,
    kind: MessageKind,
    content: String,
    wrapped_lines: Vec<String>, // Pre-wrapped lines
    height: u16,                // Calculated height
}

// Calculate message dimensions
fn calculate_message_height(content: &str, width: u16) -> u16 {
    let wrapped = textwrap::wrap(content, width as usize);
    wrapped.len() as u16
}

// ---------- helpers ----------------------------------------------------------
#[instrument(skip(content), level = "trace")]
fn calc_height(content: &str, width: u16) -> u16 {
    textwrap::wrap(content, width as usize).len() as u16
}

/// Returns `(lines_consumed, Vec<Line<'a>>)` borrowing the wrapped text
#[instrument(skip(content))]
fn render_one_message<'a>(
    content: &'a str,
    width: u16,
    style: Style,
    selected: bool,
) -> (u16, Vec<Line<'a>>) {
    let wrapped = textwrap::wrap(content, width.saturating_sub(2) as usize);
    let bar = Span::styled("│", style.fg(Color::White));

    let lines: Vec<Line<'a>> = wrapped
        .into_iter()
        .map(|s| {
            let mut spans = Vec::with_capacity(2);
            if selected {
                spans.push(bar.clone());
            }
            spans.push(Span::raw(s));
            Line::from(spans)
        })
        .collect();

    (lines.len() as u16, lines)
}
// ---------- main replacement -------------------------------------------------
#[instrument(skip(app, frame, renderable_msg), level = "trace")]
pub fn render_messages(
    app: &mut App,
    frame: &mut Frame,
    renderable_msg: &[RenderableMessage],
    conversation_width: u16,
    conversation_area: Rect,
    offset_y: u16,
) -> (u16, Vec<u16>) {
    // ------------------------------------------------------------------
    // 1. Compute per-message height and total virtual height
    // ------------------------------------------------------------------
    let viewport_height = conversation_area.height;
    let selected_index = app
        .list
        .selected()
        .unwrap_or_else(|| renderable_msg.len().saturating_sub(1));

    let mut heights: Vec<u16> = Vec::with_capacity(renderable_msg.len());
    let mut total_height = 0u16;
    for (idx, msg) in renderable_msg.iter().enumerate() {
        let eff_w = conversation_width.saturating_sub(if idx == selected_index { 1 } else { 0 });
        let h = calc_height(&msg.content, eff_w);
        heights.push(h);
        total_height = total_height.saturating_add(h);
    }

    // ------------------------------------------------------------------
    // 2. Use external scroll offset (clamped to content)
    // ------------------------------------------------------------------
    let clamped_offset_y = offset_y.min(total_height.saturating_sub(viewport_height));

    // ------------------------------------------------------------------
    // 3. Render visible slice directly
    // ------------------------------------------------------------------
    let mut y_screen = 0u16;
    let mut y_virtual = 0u16;

    for (idx, msg) in renderable_msg.iter().enumerate() {
        let height = heights[idx];
        let is_selected = idx == selected_index;
        let base_style = match msg.kind {
            MessageKind::User => Style::new().blue(),
            MessageKind::Assistant => Style::new().green(),
            MessageKind::System => Style::new().cyan(),
            MessageKind::SysInfo => Style::new().magenta(),
            _ => Style::new().white(),
        };

        if y_virtual + height <= clamped_offset_y {
            y_virtual += height;
            continue;
        }

        // Use the same effective width as in height calc (subtract 1 for bar when selected)
        let eff_w = conversation_width.saturating_sub(if is_selected { 1 } else { 0 });
        let wrapped = textwrap::wrap(&msg.content, eff_w as usize);
        let bar = Span::styled("│", base_style.fg(Color::White));
        // If offset lands inside this message, skip top lines so we don’t waste space
        let mut start_line = 0usize;
        if clamped_offset_y > y_virtual {
            start_line = (clamped_offset_y - y_virtual) as usize;
        }
        for (line_idx, line) in wrapped.iter().enumerate().skip(start_line) {
            let mut spans = Vec::with_capacity(2);
            if is_selected {
                spans.push(bar.clone());
            }
            spans.push(Span::raw(line.as_ref()));
            let para = Paragraph::new(Line::from(spans)).style(base_style);

            let area = Rect::new(
                conversation_area.x + 1,
                conversation_area.y + y_screen,
                conversation_width,
                1,
            );
            frame.render_widget(para, area);
            y_screen += 1;
            if y_screen >= viewport_height {
                return (total_height, heights);
            }
        }
        y_virtual += height;
    }
    (total_height, heights)
}
