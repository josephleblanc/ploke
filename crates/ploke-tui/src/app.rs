use tokio::sync::RwLock;

use crate::{
    app_state::ListNavigation,
    chat_history::{Message, MessageStatus, Role},
};

use super::*;

#[derive(Default, Copy, Clone, PartialEq, Eq, Debug)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Command,
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

            let renderable_messages = current_path
                .iter()
                .map(|m| RenderableMessage {
                    id: m.id,
                    role: m.role,
                    content: m.content.clone(),
                })
                .collect::<Vec<RenderableMessage>>();
            drop(history_guard);

            // 2. Draw the UI with the prepared data.
            terminal.draw(|frame| self.draw(frame, &renderable_messages, current_id))?;

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
    fn draw(&mut self, frame: &mut Frame, path: &[RenderableMessage], current_id: Uuid) {
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
        let conversation_width = main_layout[0].width.saturating_sub(6);

        // Wrap text and create ListItems
        let messages: Vec<ListItem> = path
            .iter()
            .map(|msg| {
                let wrapped_text: String =
                    textwrap::fill(&msg.content, conversation_width as usize);
                match msg.role {
                    Role::User => ListItem::new(wrapped_text).blue(),
                    Role::Assistant => ListItem::new(wrapped_text).green(),
                    Role::System => ListItem::new(wrapped_text).magenta(),
                }
                // ListItem::new(wrapped_text)
            })
            .collect();

        let list_len = messages.len();
        let list = List::new(messages)
            .block(Block::bordered().title("Conversation"))
            .highlight_symbol(">>");
        // .repeat_highlight_symbol(true);

        let list = match self.mode {
            Mode::Normal => list.highlight_style(Style::new().bg(Color::DarkGray)),
            _ => list
        };
        // Render input area
        let input = Paragraph::new(self.input_buffer.as_str())
            .block(Block::bordered().title("Input"))
            .style(match self.mode {
                Mode::Normal => Style::default(),
                Mode::Insert => Style::default().fg(Color::Yellow),
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

    /// Synchronizes the UI's list selection with the currently selected message in AppState.
    ///
    /// This is an `async` function because it needs to acquire a read lock on the
    /// shared `AppState`.
    /// This changes in reaction to the change in the state of the `AppState`.
    async fn sync_list_selection(&mut self) {
        // Acquire a read lock on the chat history.
        let guard = self.state.chat.0.read().await;

        // Get the current path of messages from the single source of truth.
        let path = guard.get_full_path();

        if let Some(current_index) = path.iter().position(|msg| msg.id == guard.current) {
            self.list.select(Some(current_index));
        } else {
            // If the current message isn't in the path for some reason, select nothing.
            self.list.select(None);
        }
    } // The read lock `guard` is dropped here.

    /// Handles the key events and updates the state of [`App`]
    fn on_key_event(&mut self, key: KeyEvent) {
        // Global quit command - this is a UI-local action
        // Question: Why is this a UI-local action? Shouldn't this send a message to the rest of
        // the application to shut down, e.g. the other tokio runtimes?
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
            self.quit();
            return;
        }

        match self.mode {
            Mode::Normal => self.handle_normal_mode(key),
            Mode::Insert => self.handle_insert_mode(key),
            Mode::Command => self.handle_command_mode(key),
        }
    }

    fn handle_insert_mode(&mut self, key: KeyEvent) {
        match key.code {
            // 1. UI-Local State Change: Switch mode
            KeyCode::Esc => self.mode = Mode::Normal,

            // 2. Shared State Change: Send a command
            KeyCode::Enter => {
                if !self.input_buffer.is_empty() {
                    self.send_cmd(StateCommand::AddUserMessage {
                        // TODO: `input_buffer` doesn't need to be cloned, try to `move` it or something
                        // instead.
                        content: self.input_buffer.clone(),
                    });
                    // Clear the UI-local buffer after sending the command
                    self.input_buffer.clear();
                }
            }

            // 3. UI-Local State Change: Modify input buffer
            KeyCode::Char(c) => self.input_buffer.push(c),
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            _ => {}
        }
    }

    fn handle_command_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.input_buffer.clear();
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                self.execute_command();
                self.input_buffer.clear();
                self.mode = Mode::Normal;
            }
            KeyCode::Char(c) => self.input_buffer.push(c),
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            _ => {}
        }
    }

    fn execute_command(&mut self) {
        let cmd = self.input_buffer.clone();
        match cmd.trim() {
            ":index" => self.send_cmd(StateCommand::IndexWorkspace),
            cmd => {
                // Placeholder for command error handling
                eprintln!("Unknown command: {}", cmd);
            }
        }
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) {
        use chat_history::NavigationDirection::{Next, Previous};

        match key.code {
            KeyCode::Char('q') => self.quit(),
            KeyCode::Char('i') => self.mode = Mode::Insert,

            // --- NAVIGATION ---
            // Send commands instead of calling local methods
            KeyCode::Char('k') | KeyCode::Up => {
                self.send_cmd(StateCommand::NavigateList {
                    direction: ListNavigation::Up,
                });
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.send_cmd(StateCommand::NavigateList {
                    direction: ListNavigation::Down,
                });
            }
            KeyCode::Char('K') => {
                // Shift-K for Top
                self.send_cmd(StateCommand::NavigateList {
                    direction: ListNavigation::Top,
                });
            }
            KeyCode::Char('J') => {
                // Shift-J for Bottom
                self.send_cmd(StateCommand::NavigateList {
                    direction: ListNavigation::Bottom,
                });
            }
            KeyCode::Char('h') | KeyCode::Left => self.send_cmd(StateCommand::NavigateBranch {
                direction: Previous,
            }),
            KeyCode::Char('l') | KeyCode::Right => {
                self.send_cmd(StateCommand::NavigateBranch { direction: Next });
            }

            // --- COMMANDS ---
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.input_buffer = ":".to_string();
            }
            _ => {}
        }
    }

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }
}

#[derive(Debug, Clone)]
struct RenderableMessage {
    id: Uuid,
    role: Role,
    content: String, // Add other fields if needed for drawing, e.g. status
}

fn truncate_uuid(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}
