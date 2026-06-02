use proptest::prelude::*;
use ratatui::layout::Rect;
use ratatui::{Terminal, backend::TestBackend};
use textwrap;

use ploke_tui::app::types::Mode;
use ploke_tui::app::view::components::input_box::InputView;
use ploke_tui::ui_theme::UiTheme;

proptest! {
    #[test]
    fn input_view_cursor_and_scroll_are_bounded(
        buffer in "[a-zA-Z0-9 \n]{0,400}",
        width in 4u16..80u16,
        height in 3u16..30u16,
    ) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let area = Rect::new(0, 0, width, height);
        let theme = UiTheme::default();
        let mut view = InputView::default();

        terminal
            .draw(|frame| {
                view.render(
                    frame,
                    area,
                    &buffer,
                    Mode::Insert,
                    &theme,
                    None,
                    &[],
                    None,
                );
            })
            .expect("draw");

        let (cursor_row, cursor_col, vscroll) = view.test_state();
        let input_height = view.desired_height(&buffer, width).min(height);
        let inner_height = input_height.saturating_sub(2).max(1);
        let input_width = width.saturating_sub(2).max(1);

        let wrapped = textwrap::wrap(&buffer, input_width as usize);
        let total_lines = wrapped.len() as u16;
        let tail_segment = buffer.rsplit('\n').next().unwrap_or("");
        let trailing_spaces = tail_segment.chars().rev().take_while(|&c| c == ' ').count() as u16;
        let extra_rows = if input_width > 0 {
            trailing_spaces / input_width
        } else {
            0
        };

        prop_assert!(cursor_col < input_width);
        prop_assert!(cursor_row >= vscroll);
        if total_lines > inner_height {
            prop_assert!(vscroll <= total_lines.saturating_sub(inner_height));
        } else {
            prop_assert_eq!(vscroll, 0);
        }
        prop_assert!(cursor_row <= total_lines.saturating_add(extra_rows));
    }
}
