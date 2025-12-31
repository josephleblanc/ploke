use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, StatefulWidget, Widget, Wrap};

/// Minimal interface for expandable list items.
///
/// Implementations should return stable title/detail lines; expansion determines
/// if detail lines are rendered and counted in layout.
///
/// # Examples
///
/// ```
/// use ratatui::text::Line;
/// use ploke_tui::app::view::widgets::expanding_list::ExpandingItem;
///
/// struct Item {
///     title: &'static str,
///     details: Vec<&'static str>,
///     expanded: bool,
/// }
///
/// impl ExpandingItem for Item {
///     fn title_line(&self) -> Line<'_> {
///         Line::from(self.title)
///     }
///
///     fn detail_lines(&self) -> Vec<Line<'_>> {
///         self.details.iter().map(|d| Line::from(*d)).collect()
///     }
///
///     fn is_expanded(&self) -> bool {
///         self.expanded
///     }
/// }
/// ```
pub trait ExpandingItem {
    /// Title line rendered for every item.
    fn title_line(&self) -> Line<'_>;
    /// Detail lines rendered only when expanded.
    fn detail_lines(&self) -> Vec<Line<'_>>;
    /// Whether to render detail lines.
    fn is_expanded(&self) -> bool;

    /// Returns detail line count without forcing allocation if overridden.
    fn detail_line_count(&self) -> usize {
        self.detail_lines().len()
    }
}

/// Stateful selection/scroll tracking for an expanding list.
///
/// # Examples
///
/// ```
/// use ploke_tui::app::view::widgets::expanding_list::ExpandingListState;
///
/// let mut state = ExpandingListState::default();
/// state.selected = 1;
/// ```
#[derive(Default, Debug, Clone)]
pub struct ExpandingListState {
    pub selected: usize,
    pub vscroll: u16,
    pub viewport_height: u16,
}

/// A stateful expanding list widget.
///
/// The widget does not own items; callers provide an item slice and the
/// `ExpandingListState` for scroll/selection tracking.
///
/// # Examples
///
/// ```
/// use ratatui::style::Style;
/// use ratatui::text::Line;
/// use ploke_tui::app::view::widgets::expanding_list::{ExpandingItem, ExpandingList};
///
/// struct Item(bool);
/// impl ExpandingItem for Item {
///     fn title_line(&self) -> Line<'_> { Line::from("title") }
///     fn detail_lines(&self) -> Vec<Line<'_>> { vec![Line::from("detail")] }
///     fn is_expanded(&self) -> bool { self.0 }
/// }
///
/// let items = vec![Item(true), Item(false)];
/// let _widget = ExpandingList {
///     items: &items,
///     normal_style: Style::default(),
///     detail_style: Style::default(),
///     selected_style: Style::default(),
/// };
/// ```
pub struct ExpandingList<'a, T> {
    pub items: &'a [T],
    pub normal_style: Style,
    pub detail_style: Style,
    pub selected_style: Style,
}

impl<'a, T> ExpandingList<'a, T>
where
    T: ExpandingItem,
{
    /// Returns detail line count for an item if expanded, otherwise 0.
    pub fn detail_lines(item: &T) -> usize {
        if !item.is_expanded() {
            return 0;
        }
        item.detail_line_count()
    }

    /// Returns total number of visible lines for the list.
    pub fn total_lines(items: &[T]) -> usize {
        items
            .iter()
            .map(|it| 1 + Self::detail_lines(it))
            .sum()
    }

    /// Returns the top line index for the selected item.
    pub fn focus_line(items: &[T], selected: usize) -> usize {
        let sel_idx = selected.min(items.len().saturating_sub(1));
        let mut line = 0usize;
        for j in 0..sel_idx {
            let it = &items[j];
            line += 1 + Self::detail_lines(it);
        }
        line
    }

    /// Computes a scroll offset that keeps the selection visible.
    pub fn compute_scroll(area: Rect, items: &[T], state: &mut ExpandingListState) {
        state.viewport_height = area.height;
        let total = Self::total_lines(items);
        let focus = Self::focus_line(items, state.selected);
        let vh = state.viewport_height as usize;
        let max_v = total.saturating_sub(vh);

        state.vscroll = (state.vscroll as usize).min(max_v) as u16;

        let v = state.vscroll as usize;
        if focus < v {
            state.vscroll = focus as u16;
        } else if focus >= v + vh {
            state.vscroll = (focus + 1).saturating_sub(vh) as u16;
        }

        if !items.is_empty() {
            let sel_idx = state.selected.min(items.len().saturating_sub(1));
            let sel = &items[sel_idx];
            if sel.is_expanded() {
                let mut block_top = 0usize;
                for j in 0..sel_idx {
                    block_top += 1 + Self::detail_lines(&items[j]);
                }
                let block_height = 1 + Self::detail_lines(sel);
                let block_bottom = block_top + block_height;
                let v = state.vscroll as usize;
                if block_bottom > v + vh {
                    let target = block_bottom.saturating_sub(vh);
                    state.vscroll = target.min(max_v) as u16;
                }
            }
        }
    }
}

impl<'a, T> StatefulWidget for ExpandingList<'a, T>
where
    T: ExpandingItem,
{
    type State = ExpandingListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        Self::compute_scroll(area, self.items, state);

        let mut lines = Vec::new();
        for (idx, item) in self.items.iter().enumerate() {
            let is_selected = idx == state.selected;
            let title_style = if is_selected {
                self.selected_style
            } else {
                self.normal_style
            };
            let detail_style = if is_selected {
                self.selected_style
            } else {
                self.detail_style
            };
            let mut title = item.title_line();
            title.style = title_style;
            lines.push(title);

            if item.is_expanded() {
                for mut detail in item.detail_lines() {
                    detail.style = detail_style;
                    lines.push(detail);
                }
            }
        }

        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((state.vscroll, 0));
        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestItem {
        title: &'static str,
        details: &'static [&'static str],
        expanded: bool,
    }

    impl ExpandingItem for TestItem {
        fn title_line(&self) -> Line<'_> {
            Line::from(self.title)
        }

        fn detail_lines(&self) -> Vec<Line<'_>> {
            self.details.iter().map(|d| Line::from(*d)).collect()
        }

        fn is_expanded(&self) -> bool {
            self.expanded
        }
    }

    #[test]
    fn total_lines_accounts_for_expansion() {
        let items = vec![
            TestItem {
                title: "a",
                details: &["a1", "a2"],
                expanded: true,
            },
            TestItem {
                title: "b",
                details: &["b1"],
                expanded: false,
            },
        ];
        assert_eq!(ExpandingList::<TestItem>::total_lines(&items), 4);
    }

    #[test]
    fn focus_line_skips_prior_blocks() {
        let items = vec![
            TestItem {
                title: "a",
                details: &["a1", "a2"],
                expanded: true,
            },
            TestItem {
                title: "b",
                details: &["b1"],
                expanded: true,
            },
        ];
        assert_eq!(ExpandingList::<TestItem>::focus_line(&items, 0), 0);
        assert_eq!(ExpandingList::<TestItem>::focus_line(&items, 1), 3);
    }

    #[test]
    fn compute_scroll_clamps_to_fit() {
        let items = vec![
            TestItem {
                title: "a",
                details: &["a1", "a2"],
                expanded: true,
            },
            TestItem {
                title: "b",
                details: &["b1", "b2", "b3"],
                expanded: true,
            },
        ];
        let area = Rect::new(0, 0, 10, 3);
        let mut state = ExpandingListState {
            selected: 1,
            vscroll: 0,
            viewport_height: 0,
        };
        ExpandingList::<TestItem>::compute_scroll(area, &items, &mut state);
        assert!(state.vscroll > 0);
        assert_eq!(state.viewport_height, 3);
    }
}
