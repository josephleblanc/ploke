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

// This function is causing us problems.
// Currently it seems to work at first, but the conversatino window runs into the user input box
// and when it reaches the bottom of the screen it crashes the program.
// The goal is for the application to have something like this:
// ____________________________
// |                          |
// | conversation history     |
// |                          |
// |                          |
// |                          |
// |                          |
// |                          |
// |                          |
// |                          |
// |--------------------------|
// | user intput              |
// |                          |
// |                          |
// |__________________________|
// The conversation history should stay within its own box and not run into the user input below,
// And when a new message is added to the conversation that would cause the history to run over
// into the user input, instead it scrolls down to keep the most recent messages in mind.
// In normal mode the the user should be able to navigate through the conversation history with the
// arrow keys and the `j` and `k` keys like in vim, where each navigation move should select the
// next message - not just the next line.
// Suppose the user has been chatting with the AI for some time and the conversation history is
// long, maybe 100 times the length of the window for the conversation history, then as the user
// selects messages by pressing `Up` or `k`, then they should begin scrolling through the earlier
// messages once they at the top of the conversation history window.
// Let's try to figure out the correct implementation here, and write some tests that will help
// verify and provide logging so we have observability of the issues here. Try to use the
// `instrument` macro where possible to make the logging less intrusive to the code flow.
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
) {
    // ------------------------------------------------------------------
    // 1. Compute per-message height and total virtual height
    // ------------------------------------------------------------------
    let mut heights: Vec<u16> = Vec::with_capacity(renderable_msg.len());
    let mut total_height = 0u16;
    for msg in renderable_msg {
        let h = calc_height(&msg.content, conversation_width);
        heights.push(h);
        total_height += h;
    }
    let viewport_height = conversation_area.height;

    // ------------------------------------------------------------------
    // 2. Scroll so the selected message is in view
    // ------------------------------------------------------------------
    let selected_index = app.list.selected().unwrap_or(0);

    let mut offset_y = 0u16;
    for (idx, &h) in heights.iter().enumerate() {
        if idx == selected_index {
            let half = viewport_height / 2;
            offset_y = offset_y.saturating_sub(half);
            break;
        }
        offset_y += h;
    }
    offset_y = offset_y.min(total_height.saturating_sub(viewport_height));

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

        if y_virtual + height <= offset_y {
            y_virtual += height;
            continue;
        }

        let wrapped = textwrap::wrap(&msg.content, conversation_width as usize);
        let bar = Span::styled("│", base_style.fg(Color::White));

        for (line_idx, line) in wrapped.iter().enumerate() {
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
                return;
            }
        }
        y_virtual += height;
    }
}
