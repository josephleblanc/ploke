use std::rc::Rc;

use super::*;
use crate::app::animation::{AnimationState, AnimationUtils};
use crate::app::types::RenderMsg;
use crate::chat_history::MessageKind;
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
#[instrument(skip(renderable_msg), level = "trace")]
pub fn measure_messages<'a, I, T: RenderMsg + 'a>(
    renderable_msg: I,
    conversation_width: u16,
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
        let eff_w = conversation_width.saturating_sub(1);
        let h = calc_height(msg.content(), eff_w);
        heights.push(h);
        total_height = total_height.saturating_add(h);
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
    animation_state: &AnimationState,
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
        let base_style = match msg.kind() {
            MessageKind::User => Style::new().blue(),
            MessageKind::Assistant => Style::new().green(),
            MessageKind::System => Style::new().cyan(),
            MessageKind::SysInfo => Style::new().magenta(),
            _ => Style::new().white(),
        };

        if y_virtual + height <= clamped_offset_y {
            y_virtual = y_virtual.saturating_add(height);
            continue;
        }

        // Use the same effective width as in height calc: always reserve 1-column gutter.
        let eff_w = conversation_width.saturating_sub(1);

        // Apply animation effects if the message has an active animation
        let content = if animation_state.has_animation(&msg.id()) {
            let progress = animation_state.get_progress(&msg.id()).unwrap_or(1.0);
            let animation_type = animation_state.get_animation_type(&msg.id()).unwrap();

            match animation_type {
                animation::AnimationType::FadeIn { .. } => {
                    AnimationUtils::apply_fade_effect(msg.content(), progress)
                }
                animation::AnimationType::Typewriter { .. } => {
                    AnimationUtils::apply_typewriter_effect(msg.content(), progress)
                }
                animation::AnimationType::SlideIn { direction, .. } => {
                    AnimationUtils::apply_slide_effect(msg.content(), progress, *direction)
                }
                animation::AnimationType::Pulse { .. } => {
                    AnimationUtils::apply_pulse_effect(msg.content(), progress, 0.5)
                }
                animation::AnimationType::Highlight { .. } => {
                    AnimationUtils::apply_highlight_effect(msg.content(), progress, "yellow")
                }
            }
        } else {
            msg.content().to_string()
        };

        let wrapped = textwrap::wrap(&content, eff_w as usize);
        let bar = Span::styled("│", base_style.fg(Color::White));

        // If offset lands inside this message, skip top lines so we don’t waste space
        let mut start_line = 0usize;
        if clamped_offset_y > y_virtual {
            start_line = (clamped_offset_y - y_virtual) as usize;
        }
        for line in wrapped.iter().skip(start_line) {
            let mut spans = Vec::with_capacity(2);
            if is_selected {
                spans.push(bar.clone());
            }

            // Apply animation-specific styling
            let mut line_style = base_style;
            if animation_state.has_animation(&msg.id()) {
                let progress = animation_state.get_progress(&msg.id()).unwrap_or(1.0);
                let animation_type = animation_state.get_animation_type(&msg.id()).unwrap();

                match animation_type {
                    animation::AnimationType::Pulse { intensity, .. } => {
                        // Add pulse effect by modulating the style
                        let pulse_strength = (1.0 + intensity * (progress * 2.0 - 1.0)).max(0.5);
                        line_style = line_style.add_modifier(Modifier::BOLD);
                    }
                    animation::AnimationType::Highlight { color, .. } => {
                        // Add highlight effect
                        line_style = line_style.fg(Color::Yellow);
                    }
                    _ => {}
                }
            }

            spans.push(Span::styled(line.as_ref(), line_style));
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
        y_virtual = y_virtual.saturating_add(height);
    }
}
