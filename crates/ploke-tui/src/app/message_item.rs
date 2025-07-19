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

/// Returns `(lines_consumed, Vec<RenderedLine>)`
#[instrument(skip(content))]
fn render_one_message(
    content: &str,
    width: u16,
    style: Style,
    selected: bool,
) -> (u16, Vec<Line<'_>>) {
    let wrapped = textwrap::wrap(content, width.saturating_sub(2) as usize);
    let bar = Span::styled("│", style.fg(Color::DarkGray));

    let lines: Vec<Line> = wrapped
        .into_iter()
        .map(|s| {
            let mut spans = Vec::with_capacity(2);
            if selected {
                spans.push(bar.clone());
            }
            spans.push(Span::styled(s, style));
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
    // 1. Build a vector of (height, lines, style) for every message
    // ------------------------------------------------------------------
    #[derive(Debug)]
    struct LineGroup {
        height: u16,
        lines: Vec<Line<'static>>,
        style: Style,
    }

    let mut groups: Vec<LineGroup> = Vec::with_capacity(renderable_msg.len());

    for msg in renderable_msg {
        let is_selected = Some(groups.len()) == app.list.selected();
        let base_style = match msg.kind {
            MessageKind::User => Style::new().blue(),
            MessageKind::Assistant => Style::new().green(),
            MessageKind::System => Style::new().cyan(),
            MessageKind::SysInfo => Style::new().magenta(),
            _ => Style::new().white(),
        };
        let style = if is_selected {
            base_style.bg(Color::DarkGray)
        } else {
            base_style
        };

        let (h, lines) = render_one_message(&msg.content, conversation_width, style, is_selected);
        groups.push(LineGroup {
            height: h,
            lines,
            style,
        });
    }

    // ------------------------------------------------------------------
    // 2. Compute virtual height of the entire buffer
    // ------------------------------------------------------------------
    let total_height: u16 = groups.iter().map(|g| g.height).sum();
    let viewport_height = conversation_area.height;

    // ------------------------------------------------------------------
    // 3. Work out the scroll offset so that the *selected* message
    //    is roughly in the middle of the viewport (unless we are near top/bottom)
    // ------------------------------------------------------------------
    let selected_index = app.list.selected().unwrap_or(0);

    let mut offset_y = 0u16;
    for (idx, g) in groups.iter().enumerate() {
        if idx == selected_index {
            let half = viewport_height / 2;
            offset_y = offset_y.saturating_sub(half);
            break;
        }
        offset_y = offset_y.saturating_add(g.height);
    }

    // clamp between 0 and (total_height - viewport_height) or 0 if smaller
    let max_offset = total_height.saturating_sub(viewport_height);
    let offset_y = offset_y.min(max_offset);

    // ------------------------------------------------------------------
    // 4. Render into the fixed conversation_area
    // ------------------------------------------------------------------
    let mut y_virtual = 0u16; // y in the virtual buffer
    let mut y_screen = 0u16; // y in the actual viewport

    for group in &groups {
        if y_virtual + group.height > offset_y {
            // at least part of this group is visible
            let mut lines_to_skip = offset_y.saturating_sub(y_virtual);
            let mut lines_to_draw = group.height.saturating_sub(lines_to_skip);

            // but we may also hit bottom of viewport
            lines_to_draw = lines_to_draw.min(viewport_height.saturating_sub(y_screen));

            for line in &group.lines[lines_to_skip as usize..][..lines_to_draw as usize] {
                let para = Paragraph::new(line.clone()).style(group.style);
                let area = Rect::new(
                    conversation_area.x + 1,
                    conversation_area.y + y_screen,
                    conversation_width,
                    1,
                );
                frame.render_widget(para, area);
                y_screen += 1;
                if y_screen >= viewport_height {
                    break;
                }
            }
        }
        y_virtual = y_virtual.saturating_add(group.height);
        if y_screen >= viewport_height {
            break;
        }
    }
}
