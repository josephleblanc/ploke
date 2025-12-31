use super::*;
use crate::app::types::RenderMsg;
use crate::app::view::rendering::highlight::{StyledSpan, highlight_message_lines};
use crate::chat_history::{AnnotationKind, MessageKind};
use crate::tools::ToolVerbosity;
use crate::tools::Audience;
use ploke_core::tool_types::ToolName;
use std::collections::HashMap;
use uuid::Uuid;

fn base_style_for_kind(kind: MessageKind) -> Style {
    match kind {
        MessageKind::User => Style::new().blue(),
        MessageKind::Assistant => Style::new().green(),
        MessageKind::System => Style::new().cyan(),
        MessageKind::SysInfo => Style::new().magenta(),
        MessageKind::Tool => Style::new().green().dim(),
    }
}

#[instrument(skip(renderable_msg), level = "trace")]
pub fn measure_messages<'a, I, T: RenderMsg + 'a>(
    renderable_msg: I,
    conversation_width: u16,
    tool_verbosity: ToolVerbosity,
    _selected_index: Option<usize>,
) -> (u16, Vec<u16>)
where
    I: IntoIterator<Item = &'a T>,
{
    // Compute per-message heights and total height for the current frame.
    let mut heights: Vec<u16> = Vec::new();
    let mut total_height = 0u16;
    for msg in renderable_msg.into_iter() {
        // Always reserve a 1-column gutter for the selection bar to keep heights stable.
        let eff_w = conversation_width.saturating_sub(1).max(1);
        let content = render_message_content(msg, tool_verbosity);
        let lines = highlight_message_lines(&content, base_style_for_kind(msg.kind()), eff_w);
        let annotation_lines = render_annotation_lines(msg, eff_w);
        let h = lines.len().saturating_add(annotation_lines.len()) as u16;
        let mut height = h.max(1);
        
        if let Some(payload) = msg.tool_payload() {
            if should_render_tool_buttons(payload) {
                height += 1;
            }
        }
        
        heights.push(height);
        total_height = total_height.saturating_add(height);
    }
    (total_height, heights)
}

#[instrument(skip(frame, renderable_msg, heights), level = "trace")]
pub fn render_messages<'a, I, T: RenderMsg + 'a>(
    frame: &mut Frame,
    renderable_msg: I,
    conversation_width: u16,
    conversation_area: Rect,
    offset_y: u16,
    heights: &[u16],
    selected_index: Option<usize>,
    tool_verbosity: ToolVerbosity,
    confirmation_states: &HashMap<Uuid, bool>,
) where
    I: IntoIterator<Item = &'a T>,
{
    // 1) Clamp offset
    let viewport_height = conversation_area.height;
    let total_height: u16 = heights
        .iter()
        .copied()
        .fold(0u16, |acc, h| acc.saturating_add(h));
    let clamped_offset_y = offset_y.min(total_height.saturating_sub(viewport_height));

    // 2) Render visible slice
    let mut y_screen = 0u16;
    let mut y_virtual = 0u16;

    for (idx, msg) in renderable_msg.into_iter().enumerate() {
        let height = heights[idx];
        let is_selected = selected_index == Some(idx);
        let base_style = base_style_for_kind(msg.kind());

        if y_virtual + height <= clamped_offset_y {
            y_virtual = y_virtual.saturating_add(height);
            continue;
        }

        // Use the same effective width as in height calc: always reserve 1-column gutter.
        let eff_w = conversation_width.saturating_sub(1).max(1);
        let content = render_message_content(msg, tool_verbosity);
        let wrapped = highlight_message_lines(&content, base_style, eff_w);
        let annotation_lines = render_annotation_lines(msg, eff_w);
        let annotation_count = annotation_lines.len();
        let bar = Span::styled("│", base_style.fg(Color::White));

        // If offset lands inside this message, skip top lines so we don’t waste space
        let mut start_line = 0usize;
        if clamped_offset_y > y_virtual {
            start_line = (clamped_offset_y - y_virtual) as usize;
        }

        let mut remaining_skip = start_line;
        let content_lines = wrapped.len();
        for line in wrapped.into_iter() {
            if remaining_skip > 0 {
                remaining_skip -= 1;
                continue;
            }
            let mut spans = Vec::with_capacity(line.len() + 1);
            if is_selected {
                spans.push(bar.clone());
            }
            append_spans(&mut spans, line);
            let para = Paragraph::new(Line::from(spans)).style(base_style);

            let area = Rect::new(
                conversation_area.x + 1,
                conversation_area.y + y_screen,
                conversation_width,
                1,
            );
            frame.render_widget(para, area);
            y_screen = y_screen.saturating_add(1);
            if y_screen >= viewport_height {
                return;
            }
        }

        for line in annotation_lines.into_iter() {
            if remaining_skip > 0 {
                remaining_skip -= 1;
                continue;
            }
            let mut spans = Vec::with_capacity(line.len() + 1);
            if is_selected {
                spans.push(bar.clone());
            }
            append_spans(&mut spans, line);
            let para = Paragraph::new(Line::from(spans));

            let area = Rect::new(
                conversation_area.x + 1,
                conversation_area.y + y_screen,
                conversation_width,
                1,
            );
            frame.render_widget(para, area);
            y_screen = y_screen.saturating_add(1);
            if y_screen >= viewport_height {
                return;
            }
        }
        
        // Render buttons if applicable
        if let Some(payload) = msg.tool_payload() {
            if should_render_tool_buttons(payload) {
                let button_y_rel = content_lines.saturating_add(annotation_count) as u16;
                let button_y_abs = y_virtual.saturating_add(button_y_rel);

                if remaining_skip > 0 {
                    remaining_skip = remaining_skip.saturating_sub(1);
                } else if button_y_abs >= clamped_offset_y && y_screen < viewport_height {
                    let is_yes = confirmation_states.get(&msg.id()).copied().unwrap_or(true);

                    let active_style = Style::new().bg(Color::Blue).fg(Color::White).bold();
                    let inactive_style = Style::new().dim();

                    let yes_span = if is_yes {
                        Span::styled("[ Yes ]", active_style)
                    } else {
                        Span::styled("[ Yes ]", inactive_style)
                    };
                    let no_span = if !is_yes {
                        Span::styled("[ No ]", active_style)
                    } else {
                        Span::styled("[ No ]", inactive_style)
                    };

                    let mut spans = Vec::new();
                    if is_selected {
                        spans.push(bar.clone());
                    }
                    spans.push(yes_span);
                    spans.push(Span::raw("  "));
                    spans.push(no_span);

                    let para = Paragraph::new(Line::from(spans));
                    let area = Rect::new(
                        conversation_area.x + 1,
                        conversation_area.y + y_screen,
                        conversation_width,
                        1,
                    );
                    frame.render_widget(para, area);
                    y_screen = y_screen.saturating_add(1);
                }
            }
        }
        
        y_virtual = y_virtual.saturating_add(height);
        
        if y_screen >= viewport_height {
            return;
        }
    }
}

fn append_spans(spans: &mut Vec<Span<'static>>, line: Vec<StyledSpan>) {
    for span in line {
        spans.push(Span::styled(span.content, span.style));
    }
}

fn render_message_content<T: RenderMsg>(msg: &T, verbosity: ToolVerbosity) -> String {
    if let Some(payload) = msg.tool_payload() {
        return payload.render(verbosity);
    }
    msg.content().to_string()
}

fn annotation_style(kind: AnnotationKind) -> Style {
    match kind {
        AnnotationKind::Hint => Style::new().dim(),
        AnnotationKind::Info => Style::new().dim(),
        AnnotationKind::Warning => Style::new().fg(Color::Yellow),
    }
}

fn render_annotation_lines<T: RenderMsg>(msg: &T, eff_w: u16) -> Vec<Vec<StyledSpan>> {
    let mut out = Vec::new();
    let Some(annotations) = msg.annotations() else {
        return out;
    };
    for annotation in annotations {
        if annotation.audience != Audience::User {
            continue;
        }
        let label = match annotation.kind {
            AnnotationKind::Hint => "hint",
            AnnotationKind::Info => "info",
            AnnotationKind::Warning => "warning",
        };
        let line = format!("  {}: {}", label, annotation.text);
        let lines = highlight_message_lines(&line, annotation_style(annotation.kind), eff_w);
        out.extend(lines);
    }
    out
}

pub(crate) fn should_render_tool_buttons(payload: &crate::tools::ToolUiPayload) -> bool {
    let is_pending = tool_payload_status(payload)
        .map(|status| status.eq_ignore_ascii_case("pending"))
        .unwrap_or(true);
    matches!(payload.tool, ToolName::ApplyCodeEdit | ToolName::NsPatch)
        && payload.error.is_none()
        && payload.request_id.is_some()
        && is_pending
}

fn tool_payload_status(payload: &crate::tools::ToolUiPayload) -> Option<&str> {
    payload
        .fields
        .iter()
        .find(|field| field.name.as_ref() == "status")
        .map(|field| field.value.as_ref())
}
