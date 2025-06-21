use super::super::*;

// Add documentation for this function AI!
pub fn layout_statusline(divs: u32, area: Rect) -> std::rc::Rc<[ratatui::layout::Rect]> {
    let constraints = (1..=divs).map(|x| Constraint::Ratio(x, divs));
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area)
}
