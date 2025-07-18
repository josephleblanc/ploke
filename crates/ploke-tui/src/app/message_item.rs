use std::rc::Rc;

use super::*;
 // In app.rs, replace the List rendering with custom Paragraph-based rendering

 #[derive(Debug, Clone)]
 struct MessageItem {
     id: Uuid,
     kind: MessageKind,
     content: String,
     wrapped_lines: Vec<String>, // Pre-wrapped lines
     height: u16,               // Calculated height
 }

 // Calculate message dimensions
 fn calculate_message_height(content: &str, width: u16) -> u16 {
     let wrapped = textwrap::wrap(content, width as usize);
     wrapped.len() as u16
 }

pub fn render_messages(
    app: &mut App, 
    frame: &mut Frame, 
    renderable_msg: &[RenderableMessage], 
    layout: &[Constraint],
    conversation_width: u16,
    conversation_area: Rc<Rect>,
) {
    // In draw method, replace List with custom rendering:
    let mut y_offset = 0u16;
    let mut items = Vec::new();

    // First pass: calculate heights and positions
    for (index, msg) in renderable_msg.iter().enumerate() {
        let height = calculate_message_height(&msg.content, conversation_width);
        let is_selected = Some(index) == app.list.selected();

        let style = match msg.kind {
            MessageKind::User => if is_selected {
                Style::new().blue().bg(Color::DarkGray)
            } else {
                Style::new().blue()
            },
            MessageKind::Assistant => if is_selected {
                Style::new().green().bg(Color::DarkGray)
            } else {
                Style::new().green()
            },
            MessageKind::SysInfo => if is_selected {
                Style::new().magenta().bg(Color::DarkGray)
            } else {
                Style::new().magenta()
            },
            MessageKind::System => if is_selected {
                Style::new().cyan().bg(Color::DarkGray)
            } else {
                Style::new().cyan()
            },
            _ => if is_selected {
                Style::new().white().bg(Color::DarkGray)
            } else {
                Style::new().white() 
            }
            // ... other kinds
        };

        let wrapped = textwrap::wrap(&msg.content, conversation_width as usize);

        // Render each line of the message
        for (line_idx, line) in wrapped.iter().enumerate() {
            let paragraph = Paragraph::new(line.clone())
                .style(style);

            let area = Rect::new(
                conversation_area.x + 1,
                conversation_area.y + y_offset,
                conversation_width,
                1,
            );

            frame.render_widget(paragraph, area);
            y_offset += 1;
        }

        // Track the total height for scrolling
        items.push((msg.id, y_offset));
    }

    // Update scroll offset based on selected item
    let selected_index = app.list.selected().unwrap_or(0);
    let scroll_offset = items.iter()
        .take(selected_index)
        .map(|(_, height)| height)
        .sum::<u16>()
        .saturating_sub(conversation_area.height / 2);
    // Apply scroll offset when rendering
}
