use tokio::sync::RwLock;

use crate::{app_state::ListNavigation, chat_history::{Message, MessageStatus}};

use super::*;

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

#[derive(Debug)]
pub struct App {
    /// Is the application running?
    running: bool,
    /// Ui-specific state for the message list (scroll position, selection)
    // Question: should `ListState` be constructed each frame, or should it persist?
    list: ListState,
    /// A read-only handle to the shared application state.
    state: Arc<AppState>,
    /// A channel to send commands to the state manager.
    cmd_tx: mpsc::Sender<StateCommand>,
    /// A channel to receive broadcasted application events.
    event_rx: tokio::sync::broadcast::Receiver<AppEvent>,
    /// User input buffer
    // (add more buffers for editing other messages later?)
    input_buffer: String,
    /// Input mode for vim-like multi-modal editing experience
    mode: Mode,
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new(
        state: Arc<AppState>,
        cmd_tx: mpsc::Sender<StateCommand>,
        event_bus: &EventBus, // reference non-Arc OK because only created at startup
    ) -> Self {
        Self {
            running: false, // Will be set to true in run()
            list: ListState::default(),
            state,
            cmd_tx,
            event_rx: event_bus.subscribe(EventPriority::Realtime),
            input_buffer: String::new(),
            mode: Mode::default(),
        }
    }

    fn send_cmd(&self, cmd: StateCommand) {
        // Use try_send to prevent the UI from blocking
        if let Err(e) = self.cmd_tx.try_send(cmd) {
            eprintln!("Failed to send command: {}", e);
        }
    }

    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;
        let mut crossterm_events = crossterm::event::EventStream::new();

        // Initialize the UI selection base on the initial state.
        self.sync_list_selection().await;

        while self.running {
            // 1. Prepare data for this frame by reading from AppState.
            let history_guard = self.state.chat.0.read().await;
            let current_path = history_guard.get_full_path();
            let current_id = history_guard.current;
            drop(history_guard);

            // 2. Draw the UI with the prepared data.
            terminal.draw(|frame| self.draw(frame, &current_path, current_id))?;

            // 3. Handle all incoming events (user input, state changes).
            tokio::select! {
                // Prioritize Ui responsiveness
                biased;

                // User input
                maybe_event = crossterm_events.next().fuse() => {
                    if let Some(Ok(event)) = maybe_event {
                        match event {
                            Event::Key(key_event) =>{ self.on_key_event(key_event); }
                            // Event::FocusGained => {},
                            // Event::FocusLost => {},
                            // Event::Mouse(mouse_event) => {},
                            // Event::Paste(_) => {},
                            // Event::Resize(_, _) => {},
                            _ => {}
                        }
                    }
                }

                // Application events
                Ok(app_event) = self.event_rx.recv() => {
                    match app_event {
                        AppEvent::MessageUpdated(_) | AppEvent::UpdateFailed(_) => {
                            self.sync_list_selection().await;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Renders the user interface.
    ///
    /// This is where you add new widgets. See the following resources for more information:
    /// - <https://docs.rs/ratatui/latest/ratatui/widgets/index.html>
    /// - <https://github.com/ratatui/ratatui/tree/master/examples>
    fn draw(&mut self, frame: &mut Frame, path: &[&Message], current_id: Uuid) {
        // ---------- Define Layout ----------
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(80),
                Constraint::Percentage(20),
                Constraint::Length(1),
            ])
            .split(frame.area());

        let status_layout = layout_statusline(4, main_layout[2]);

        // ---------- Prepare Widgets ----------
        // Render message tree
        let messages: Vec<ListItem> = path.iter()
            .map(|msg| ListItem::new(Line::from(Span::raw(msg.content.clone()))))
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
            .style(match self.mode {
                Mode::Normal => Style::default(),
                Mode::Insert => Style::default().fg(Color::Yellow)
            });

        // Render Mode to text
        let status_bar = Block::default()
            .title(self.mode.to_string())
            .borders(Borders::NONE)
            .padding(Padding::vertical(1));
        let node_status = Paragraph::new(format!("Node: {}", truncate_uuid(current_id)))
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
            let new_message_id = self.chat_history.add_child(
                self.chat_history.current,
                &self.input_buffer,
                MessageStatus::Completed,
            )?;
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

    fn handle_normal_mode(&mut self, key: KeyEvent) {
        use chat_history::NavigationDirection::{Next, Previous};

        match key.code {
            KeyCode::Char('q') => self.quit(),
            KeyCode::Char('i') => self.mode = Mode::Insert,

            // --- NAVIGATION ---
            // Send commands instead of calling local methods
            KeyCode::Char('k') | KeyCode::Up => {
                self.send_cmd(StateCommand::NavigateList {direction: ListNavigation::Up });
            }
             KeyCode::Char('j') | KeyCode::Down => {
                 self.send_cmd(StateCommand::NavigateList { direction: ListNavigation::Down });
             }
             KeyCode::Char('K') => { // Shift-K for Top
                 self.send_cmd(StateCommand::NavigateList { direction: ListNavigation::Top });
             }
             KeyCode::Char('J') => { // Shift-J for Bottom
                 self.send_cmd(StateCommand::NavigateList { direction: ListNavigation::Bottom });
             }
             KeyCode::Char('h') | KeyCode::Left => {
                 self.send_cmd(StateCommand::NavigateBranch { direction: Previous })
             }
             KeyCode::Char('l') | KeyCode::Right => {
                 self.send_cmd(StateCommand::NavigateBranch { direction: Next });
             }
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
