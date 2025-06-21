// TODO:
//
// 1 Add serialization support for saving/loading conversations
// 2 Implement scrolling through long message histories
// 3 Add visual indicators for branch points
// 4 Implement sibling navigation (up/down between children of same parent)
// 5 Add color coding for different message types (user vs assistant)

mod chat_history;
mod utils;

use utils::layout::{self, layout_statusline};

use std::{collections::HashMap, thread::current};

use chat_history::{ChatError, ChatHistory, NavigationDirection};
use color_eyre::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::{FutureExt, StreamExt};
use ratatui::{
    DefaultTerminal, Frame,
    style::Stylize,
    text::Line,
    widgets::{Block, Borders, ListItem, ListState, Padding, Paragraph},
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

#[derive(Default, Copy, Clone, PartialEq, Eq, Debug)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Normal => write!(f, "Normal"),
            Mode::Insert => write!(f, "Insert"),
        }
    }
}

#[derive(Debug, Default)]
pub struct App {
    /// Is the application running?
    running: bool,
    /// Event stream.
    event_stream: EventStream,
    //
    list: ListState,
    /// Branching Chat History
    pub chat_history: ChatHistory,
    /// User input buffer
    // (add more buffers for editing other messages later?)
    input_buffer: String,
    /// Input mode for vim-like multi-modal editing experience
    mode: Mode,
    branches: Vec<Vec<Uuid>>,
    // NOTE: Potential problem/room for improvement:
    //  The branch state is tracked here via `active_branch`, and in `ChatHistory` via `current`
    //  and `selected_child`. Is this appropriate?
    //  Possible better design: Having a central `Branch` struct - but where would this go?
    active_branch: usize,
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new() -> Self {
        let chat_history = ChatHistory::new();
        let root_id = chat_history.current;
        Self {
            running: false, // Will be set to true in run()
            event_stream: EventStream::new(),
            list: ListState::default(),
            chat_history,
            input_buffer: String::new(),
            mode: Mode::default(),
            branches: vec![vec![root_id]],
            active_branch: 0
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
        // ---------- Define layout ----------
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(80),
                Constraint::Percentage(20),
                Constraint::Length(1),
            ])
            .split(frame.area());

        let status_layout = layout_statusline(4, main_layout[2]);

        // ---------- Define widgets ----------
        // Render message tree
        let messages: Vec<ListItem> = self
            .chat_history
            .get_full_path()
            .iter()
            .map(
                |msg| {
                    let parent_id = msg
                        .parent
                        .map(|id| format!("←{}", truncate_uuid(id)))
                        .unwrap_or_default();
                    ListItem::new(Line::from(vec![
                        Span::raw(&msg.content),
                        Span::styled(parent_id, Style::new().dim()),
                    ]))
                }, // old version, trying something new above
                   // ListItem::new(Line::from(vec![Span::raw(&msg.content)]))
            )
            .collect();

        let list_len = messages.len();
        let list = List::new(messages)
            .block(Block::bordered().title("Conversation"))
            .highlight_style(Style::new().reversed())
            .highlight_symbol(">>");
        // .repeat_highlight_symbol(true);

        // Render input area
        let input = Paragraph::new(self.input_buffer.as_str())
            .block(Block::bordered().title("Input"))
            .style(Style::new().fg(Color::Yellow));

        // Render Mode to text
        let status_bar = Block::default()
            .title(self.mode.to_string())
            .borders(Borders::NONE)
            .padding(Padding::vertical(1));
        let current_node = truncate_uuid(self.chat_history.current);
        let node_status = Paragraph::new(format!("Node: {}", current_node))
            .block(Block::default().borders(Borders::NONE))
            .style(Style::new().fg(Color::Blue));
        let list_len = Paragraph::new(format!("List Len: {}", list_len));
        let list_selected = Paragraph::new(format!("Selected: {:?}", self.list.selected()));

        // ---------- Render widgets in layout ----------
        // -- top level
        frame.render_stateful_widget(list, main_layout[0], &mut self.list);
        frame.render_widget(input, main_layout[1]);

        // -- first nested
        frame.render_widget(status_bar, status_layout[0]);
        frame.render_widget(node_status, status_layout[1]);
        frame.render_widget(list_len, status_layout[2]);
        frame.render_widget(list_selected, status_layout[3]);
    }

    fn create_branch(&mut self) {
        // let new_branch = self.chat_history.
    }

    fn move_selection_up(&mut self) {
        self.list.select_previous();
        if let Some(selected) = self.list.selected() {
            let path = self.chat_history.get_full_path();
            self.chat_history.current = path[selected].id; // Sync tree position
        }
    }

    fn move_selection_down(&mut self) {
        // let current_id = self.chat_history.current;
        // let current_msg = self.chat_history.messages.get(&current_id);
        // if let Some(child_id) = current_msg.map(|m| m.selected_child) {
        // };
        if let Some(selected) = self.list.selected() {
            let path = self.chat_history.get_full_path();
            self.chat_history.current = path[selected].id; // Sync tree position
        }
        self.list.select_next();
    }

    fn move_to_first(&mut self) {
        self.list.select_first();
        if let Some(selected) = self.list.selected() {
            let path = self.chat_history.get_full_path();
            self.chat_history.current = path[selected].id;
        }
    }

    fn move_to_last(&mut self) {
        self.list.select_last();
        if let Some(selected) = self.list.selected() {
            let path = self.chat_history.get_full_path();
            self.chat_history.current = path[selected].id;
        }
    }

    /// Navigates between sibling messages sharing the same parent
    ///
    /// # Arguments
    /// * `direction` - NavigationDirection::Next/Previous to move through siblings
    ///
    /// # Returns
    /// Result containing UUID of new current message if successful
    ///
    /// # Errors
    /// Returns `ChatError::RootHasNoSiblings` if trying to navigate from root
    /// Returns `ChatError::SiblingNotFound` if no siblings available
    pub fn navigate_sibling(&mut self, direction: NavigationDirection) -> Result<(), ChatError> {
        let current_id = self.chat_history.current;
        let current_msg = self
            .chat_history
            .messages
            .get(&current_id)
            .ok_or(ChatError::SiblingNotFound(current_id))?;

        let parent_id = current_msg.parent.ok_or(ChatError::RootHasNoSiblings)?;

        let parent = self
            .chat_history
            .messages
            .get(&parent_id)
            .ok_or(ChatError::ParentNotFound(parent_id))?;

        // Get all siblings including current message
        let siblings = &parent.children;
        let current_idx = siblings
            .iter()
            .position(|&id| id == current_id)
            .unwrap_or(siblings.len() - 1);

        let new_idx = match direction {
            NavigationDirection::Next => (current_idx + 1) % siblings.len(),
            NavigationDirection::Previous => {
                if current_idx == 0 {
                    siblings.len() - 1
                } else {
                    current_idx - 1
                }
            }
        };

        self.chat_history.current = siblings[new_idx];
        self.sync_list_selection();
        Ok(())
    }

    fn add_user_message_safe(&mut self) -> Result<(), chat_history::ChatError> {
        if !self.input_buffer.is_empty() {
            // let new_message_id = self
            //     .chat_history
            //     .add_child(self.chat_history.current, &self.input_buffer)?;
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
        let path = self.chat_history.get_full_path();
        if let Some(current_index) = path
            .iter()
            .position(|msg| msg.id == self.chat_history.current)
        {
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
        let mode = self.mode;

        match mode {
            Mode::Normal => match (key.modifiers, key.code) {
                (_, KeyCode::Char('i')) => self.mode = Mode::Insert,

                // navigate messages in conversation
                (_, KeyCode::Up | KeyCode::Char('k')) => self.move_selection_up(),
                (_, KeyCode::Down | KeyCode::Char('j')) => self.move_selection_down(),
                (_, KeyCode::Char('K')) => self.move_to_first(),
                (_, KeyCode::Char('J')) => self.move_to_last(),

                // Navigation
                // Sibling navigation
                (_, KeyCode::Char('h') | KeyCode::Left) => {
                    self.navigate_sibling(NavigationDirection::Previous).ok();
                }
                (_, KeyCode::Char('l') | KeyCode::Right) => {
                    self.navigate_sibling(NavigationDirection::Next).ok();
                }

                _ => {}
            },
            Mode::Insert => match (key.modifiers, key.code) {
                (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => {
                    todo!("Add way to cancel waiting for response")
                }
                (KeyModifiers::CONTROL, KeyCode::Char('j') | KeyCode::Char('k')) => {
                    self.mode = Mode::Normal;
                }
                (_, KeyCode::Esc) => self.mode = Mode::Normal,

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
                _ => {}
            },
        }
        match (key.modifiers, key.code) {
            // How to quit application
            (KeyModifiers::CONTROL, KeyCode::Char('c') | KeyCode::Char('C')) => self.quit(),

            // Add other key handlers here.
            _ => {}
        }
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }
}
fn truncate_uuid(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}
