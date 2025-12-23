use ratatui::prelude::Stylize;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Widget},
};

pub trait ExpandableItem {
    fn title(&self) -> String;
    fn is_expanded(&self) -> bool;
    fn details(&self) -> Vec<String>;
}

pub struct ExpandingList<'a, T: ExpandableItem> {
    items: &'a [T],
    selected: usize,
    style: Style,
    selected_style: Style,
    detail_style: Style,
}

// add builder methods for ergonomic usage AI!
impl<'a, T: ExpandableItem> ExpandingList<'a, T> {
    pub fn new(
        items: &'a [T],
        selected: usize,
        style: Style,
        selected_style: Style,
        detail_style: Style,
    ) -> Self {
        Self {
            items,
            selected,
            style,
            selected_style,
            detail_style,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let list_items: Vec<ListItem> = self
            .items
            .iter()
            .enumerate()
            .flat_map(|(i, item)| {
                let mut items = Vec::new();

                // Main item line
                let main_line = Line::from(vec![
                    Span::styled(
                        if i == self.selected { ">" } else { " " },
                        if i == self.selected {
                            self.selected_style
                        } else {
                            self.style
                        },
                    ),
                    Span::raw(" "),
                    Span::styled(
                        item.title(),
                        if i == self.selected {
                            self.selected_style
                        } else {
                            self.style
                        },
                    ),
                ]);
                items.push(ListItem::new(main_line));

                // Expanded details
                if item.is_expanded() {
                    for detail in item.details() {
                        items.push(ListItem::new(Line::from(Span::styled(
                            format!("    {}", detail),
                            self.detail_style,
                        ))));
                    }
                }

                items
            })
            .collect();

        let list = List::new(list_items)
            .block(Block::default().borders(Borders::ALL))
            .style(self.style);

        frame.render_widget(list, area);
    }
}

// Note: This function should be moved to the model browser component
// and use the new ExpandingList widget
