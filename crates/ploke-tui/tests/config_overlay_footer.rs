use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Terminal;

use ploke_tui::app::view::components::config_overlay::{
    ConfigOverlayState, render_config_overlay,
};
use ploke_tui::app_state::RuntimeConfig;

fn rect_lines(term: &Terminal<TestBackend>, rect: Rect) -> Vec<String> {
    let buffer = term.backend().buffer();
    let mut out = Vec::new();
    for row in rect.y..rect.y.saturating_add(rect.height) {
        let mut line = String::new();
        for col in rect.x..rect.x.saturating_add(rect.width) {
            let sym = buffer
                .cell((col, row))
                .expect("buffer cell in-bounds")
                .symbol()
                .chars()
                .next()
                .unwrap_or(' ');
            line.push(sym);
        }
        out.push(line);
    }
    out
}

fn footer_rect(area: Rect, help_visible: bool) -> Rect {
    let width = area.width.saturating_mul(8) / 10;
    let height = area.height.saturating_mul(8) / 10;
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area.y.saturating_add(area.height.saturating_sub(height) / 2);
    let rect = Rect::new(x, y, width.max(50), height.max(12));

    let footer_height = if help_visible { 6 } else { 4 };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
        .split(rect);
    layout[1]
}

fn inner_rect(rect: Rect) -> Rect {
    Rect::new(
        rect.x.saturating_add(1),
        rect.y.saturating_add(1),
        rect.width.saturating_sub(2),
        rect.height.saturating_sub(2),
    )
}

#[test]
fn config_overlay_footer_shows_summary_when_help_hidden() {
    let backend = TestBackend::new(80, 24);
    let mut term = Terminal::new(backend).expect("terminal");
    let cfg = RuntimeConfig::default();
    let overlay = ConfigOverlayState::from_runtime_config(&cfg);
    term.draw(|frame| render_config_overlay(frame, &overlay))
        .expect("render");

    let footer = inner_rect(footer_rect(Rect::new(0, 0, 80, 24), false));
    let lines = rect_lines(&term, footer);
    assert!(
        lines.iter().any(|line| line.contains("Command Style")),
        "expected command style description in footer when help is hidden"
    );
    assert!(
        lines.iter().any(|line| line.contains("Help")),
        "expected help hint in footer when help is hidden"
    );
}

#[test]
fn config_overlay_footer_shows_help_when_enabled() {
    let backend = TestBackend::new(80, 24);
    let mut term = Terminal::new(backend).expect("terminal");
    let cfg = RuntimeConfig::default();
    let mut overlay = ConfigOverlayState::from_runtime_config(&cfg);
    overlay.help_visible = true;
    term.draw(|frame| render_config_overlay(frame, &overlay))
        .expect("render");

    let footer = inner_rect(footer_rect(Rect::new(0, 0, 80, 24), true));
    let lines = rect_lines(&term, footer);
    assert!(
        lines.iter().any(|line| line.contains("Keys: tab/shift+tab")),
        "expected key help in footer when help is enabled"
    );
    assert!(
        lines.iter().any(|line| line.contains("Note: changes")),
        "expected note line in footer when help is enabled"
    );
    assert!(
        lines.iter().any(|line| line.contains("Command Style")),
        "expected command style description in footer when help is enabled"
    );
}
