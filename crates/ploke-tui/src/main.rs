// TODO:
//
// 1 Add serialization support for saving/loading conversations
// 2 Implement scrolling through long message histories
// 3 Add visual indicators for branch points
// 4 Implement sibling navigation (up/down between children of same parent)
// 5 Add color coding for different message types (user vs assistant)

mod chat_history;

use std::collections::HashMap;

use chat_history::ChatHistory;
use color_eyre::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::{FutureExt, StreamExt};
use ratatui::{
    DefaultTerminal, Frame,
    style::Stylize,
    text::Line,
    widgets::{Block, ListItem, ListState, Paragraph},
};
// for list
use ratatui::prelude::*;
use ratatui::{
    style::Style,
    widgets::{List, ListDirection},
};
use uuid::Uuid;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = App::new().run(terminal).await;
    ratatui::restore();
    result
}

#[derive(Debug, Default)]
pub struct App {
    /// Is the application running?
    running: bool,
    // Event stream.
    event_stream: EventStream,
    //
    list: ListState,
    // Branching Chat History
    pub chat_history: ChatHistory,
    // User input buffer
    // (add more buffers for editing other messages later?)
    input_buffer: String,
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new() -> Self {
        let chat_history = ChatHistory::new();
        Self {
            running: false, // Will be set to true in run()
            event_stream: EventStream::new(),
            list: ListState::default(),
            chat_history,
            input_buffer: String::new(),
        }
    }

    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;
        while self.running {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_crossterm_events().await?;
        }
        Ok(())
    }

    /// Renders the user interface.
    ///
    /// This is where you add new widgets. See the following resources for more information:
    /// - <https://docs.rs/ratatui/latest/ratatui/widgets/index.html>
    /// - <https://github.com/ratatui/ratatui/tree/master/examples>
    fn draw(&mut self, frame: &mut Frame) {
        // Define layout
        // Here just a simple 50-50 split top/bottom
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(80), Constraint::Percentage(20)])
            .split(frame.area());

        // Render message tree
        let messages: Vec<ListItem> = self
            .chat_history
            .get_current_path()
            .iter()
            .enumerate()
            .map(|(depth, msg)| {
                let indent = "  ".repeat(depth);
                let prefix = if depth == 0 {
                    "◈ ".blue()
                } else {
                    "↳ ".dark_gray()
                };

                ListItem::new(Line::from(vec![
                    Span::raw(indent),
                    prefix,
                    Span::raw(&msg.content),
                ]))
            })
            .collect();

        let list = List::new(messages)
            .block(Block::bordered().title("Conversation"))
            .highlight_style(Style::new().reversed())
            .highlight_symbol(">>");
        // .repeat_highlight_symbol(true);

        // Render input area
        let input = Paragraph::new(self.input_buffer.as_str())
            .block(Block::bordered().title("Input"))
            .style(Style::new().fg(Color::Yellow));

        frame.render_stateful_widget(list, layout[0], &mut self.list);
        frame.render_widget(input, layout[1]);
    }

    fn navigate_parent(&mut self) {
        if let Some(parent) = self.chat_history.messages[&self.chat_history.current].parent {
            self.chat_history.current = parent;
            self.sync_list_selection();
        }
    }

    fn navigate_child(&mut self) {
        let current = &self.chat_history.messages[&self.chat_history.current];
        if let Some(first_child) = current.children.first() {
            self.chat_history.current = *first_child;
            self.sync_list_selection();
        }
    }

    fn add_user_message_safe(&mut self) -> Result<(), chat_history::ChatError> {
        if !self.input_buffer.is_empty() {
            let new_message_id = self
                .chat_history
                .add_child(self.chat_history.current, &self.input_buffer)?;
            self.chat_history.current = new_message_id;
            self.input_buffer.clear();
            self.sync_list_selection();
        }
        Ok(())
    }

    fn sync_list_selection(&mut self) {
        let path = self.chat_history.get_current_path();
        if let Some(current_index) = path.iter().position(|msg| msg.id == self.chat_history.current) {
            self.list.select(Some(current_index));
        }
    }

    /// Reads the crossterm events and updates the state of [`App`].
    async fn handle_crossterm_events(&mut self) -> Result<()> {
        tokio::select! {
            event = self.event_stream.next().fuse() => {
                match event {
                    Some(Ok(evt)) => {
                        match evt {
                            Event::Key(key)
                                if key.kind == KeyEventKind::Press
                                    => self.on_key_event(key),
                            Event::Mouse(_) => {}
                            Event::Resize(_, _) => {}
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                // Sleep for a short duration to avoid busy waiting.
            }
        }
        Ok(())
    }

    /// Handles the key events and updates the state of [`App`].
    fn on_key_event(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            // How to quit application
            (_, KeyCode::Esc | KeyCode::Char('q'))
            | (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),

            // navigate messages in conversation
            (_, KeyCode::Up | KeyCode::Char('k')) => self.list.select_previous(),
            (_, KeyCode::Down | KeyCode::Char('j')) => self.list.select_next(),
            (_, KeyCode::Char('K')) => self.list.select_first(),
            (_, KeyCode::Char('J')) => self.list.select_last(),

            // Navigation
            (_, KeyCode::Left) => self.navigate_parent(),
            (_, KeyCode::Right) => self.navigate_child(),

            // Input handling
            (_, KeyCode::Char(c)) => self.input_buffer.push(c),
            (_, KeyCode::Backspace) => {
                self.input_buffer.pop();
            }
            (_, KeyCode::Enter) => {
                if let Err(e) = self.add_user_message_safe() {
                    // Could log error or show in UI
                    eprintln!("Error adding message: {}", e);
                }
            }

            // Add other key handlers here.
            _ => {}
        }
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }
}
