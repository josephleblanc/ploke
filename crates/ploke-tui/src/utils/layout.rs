use super::super::*;

/// Creates a horizontal layout split into equal ratio divisions for status line components.
///
/// # Arguments
/// * `divs` - Number of equal-width divisions to create (minimum 1)
/// * `area` - Rectangular area to divide
///
/// # Returns
/// Rc<[Rect]> containing the split areas for efficient rendering
///
/// # Example
/// ```
/// use ratatui::layout::Rect;
/// use ploke_tui::utils::layout::layout_statusline;
/// // Split a 100px wide area into 3 sections (33%, 66%, 100% of remaining space)
/// let area = Rect::new(0, 0, 10, 10);
/// let layout = layout_statusline(3, area);
/// ```
pub fn layout_statusline(divs: u32, area: Rect) -> std::rc::Rc<[ratatui::layout::Rect]> {
    let constraints = (1..=divs).map(|x| Constraint::Ratio(x, divs));
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area)
}
