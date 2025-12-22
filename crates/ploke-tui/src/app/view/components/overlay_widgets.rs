use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

pub fn render_search_bar(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    mode_label: &str,
    input: &str,
    cursor_col: Option<u16>,
    style: Style,
) {
    let prompt_prefix = format!("[{mode_label}] ");
    let input_line = Line::from(vec![
        Span::styled(prompt_prefix.as_str(), style),
        Span::styled(input, style),
    ]);
    let input_widget = Paragraph::new(input_line)
        .style(style)
        .block(Block::default().borders(Borders::ALL).title(title).style(style))
        .wrap(Wrap { trim: false });
    frame.render_widget(input_widget, area);
    if let Some(cursor_col) = cursor_col {
        let cursor_x = area.x
            + 1 // left border padding
            + UnicodeWidthStr::width(prompt_prefix.as_str()) as u16
            + cursor_col;
        let cursor_y = area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

pub fn empty_state_line<'a>(message: &'a str, style: Style) -> Line<'a> {
    Line::from(Span::styled(message, style))
}

pub fn render_diff_preview(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
) {
    let detail = Paragraph::new(lines)
        .block(Block::bordered().title(title))
        .alignment(Alignment::Left);
    frame.render_widget(detail, area);
}
