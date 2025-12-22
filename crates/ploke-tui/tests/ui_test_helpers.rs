use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tracing::warn;

pub fn render_to_buffer<F>(area: Rect, draw: F) -> Buffer
where
    F: FnOnce(&mut ratatui::Frame<'_>),
{
    let mut term = Terminal::new(TestBackend::new(area.width, area.height)).expect("terminal");
    term.draw(|f| draw(f)).expect("draw");
    term.backend().buffer().clone()
}

pub fn buffer_to_string(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let symbol = buf.cell((x, y)).expect("cell").symbol();
            let mut chars = symbol.chars();
            let ch = match chars.next() {
                Some(first) => {
                    if chars.next().is_some() {
                        warn!(x, y, symbol, "buffer cell contains multi-char symbol");
                    }
                    first
                }
                None => {
                    warn!(x, y, "buffer cell contains empty symbol");
                    ' '
                }
            };
            out.push(ch);
        }
        out.push('\n');
    }
    out
}
