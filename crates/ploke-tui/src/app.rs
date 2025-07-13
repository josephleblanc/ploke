use crate::{app_state::ListNavigation, chat_history::MessageKind, user_config::CommandStyle};

use super::*;
use std::time::{Duration, Instant};

use app_state::{AppState, StateCommand};
use color_eyre::Result;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::Gauge;

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
            Mode::Command => write!(f, "Command"),
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
    pub input_buffer: String,
    /// Input mode for vim-like multi-modal editing experience
    pub mode: Mode,
    command_style: CommandStyle,
    indexing_state: Option<indexer::IndexingStatus>,
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new(
        command_style: CommandStyle,
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
            command_style,
            indexing_state: None,
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

        let mut frame_counter = 0;
        while self.running {
            let _frame_span_guard = tracing::debug_span!("frame", number = frame_counter).entered();
            let frame_start = Instant::now();

            // 1. Prepare data for this frame by reading from AppState.
            let history_guard = self.state.chat.0.read().await;
            let current_path = history_guard.get_full_path();
            let current_id = history_guard.current;

            // TODO: See if we can avoid this `collect` somehow. Does `self.draw` take an Iterator?
            // Could it be made to?
            let renderable_messages = current_path
                .iter()
                .map(|m| RenderableMessage {
                    id: m.id,
                    kind: m.kind,
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
                            Event::FocusGained => {},
                            Event::FocusLost => {},
                            Event::Mouse(mouse_event) => {},
                            Event::Paste(_) => {},
                            Event::Resize(_, _) => {},
                        }
                    }
                }

                // Application events
                Ok(app_event) = self.event_rx.recv() => {
                    match app_event {
                        AppEvent::MessageUpdated(_)|AppEvent::UpdateFailed(_)=>{
                            self.sync_list_selection().await;
                        }
                        AppEvent::IndexingProgress(state)=>{
                            self.indexing_state = Some(state);
                        }
                        AppEvent::Ui(ui_event) => {},
                        AppEvent::Llm(event) => {},
                        AppEvent::System(system_event) => {},
                        AppEvent::Error(error_event) => {},
                        AppEvent::IndexingStarted => {},
                        AppEvent::IndexingCompleted => {
                            tracing::info!("Indexing Succeeded!");
                            self.send_cmd(StateCommand::AddMessageImmediate {
                                msg: String::from("IndexingSucceeded"),
                                kind: MessageKind::SysInfo,
                            })
                        },
                        AppEvent::IndexingFailed => {
                            tracing::error!("Indexing Failed");
                            self.send_cmd(StateCommand::AddMessageImmediate {
                                msg: String::from("Indexing Failed"),
                                kind: MessageKind::SysInfo,
                            })
                        },
                    }
                }
            }
            let frame_duration = frame_start.elapsed();
            if frame_duration > Duration::from_millis(16) {
                tracing::warn!(
                    frame_duration_ms = frame_duration.as_millis(),
                    "Frame budget exceeded"
                );
            }
            frame_counter += 1;
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
                match msg.kind {
                    MessageKind::User => ListItem::new(wrapped_text).blue(),
                    MessageKind::Assistant => ListItem::new(wrapped_text).green(),
                    MessageKind::System => ListItem::new(wrapped_text).gray(),
                    MessageKind::Tool => todo!(),
                    MessageKind::SysInfo => ListItem::new(wrapped_text).magenta(),
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
            _ => list,
        };
        // Render input area with dynamic title
        let input_title = match (self.mode, self.command_style) {
            (Mode::Command, CommandStyle::NeoVim) => "Command Mode",
            (Mode::Command, CommandStyle::Slash) => "Slash Mode",
            _ => "Input",
        };

        let input_width = main_layout[1].width.saturating_sub(2);
        let input = Paragraph::new(textwrap::fill(
            self.input_buffer.as_str(),
            input_width as usize,
        ))
        .block(Block::bordered().title(input_title))
        .style(match self.mode {
            Mode::Normal => Style::default(),
            Mode::Insert => Style::default().fg(Color::Yellow),
            Mode::Command => Style::default().fg(Color::Cyan),
        });
        // Add progress bar at bottom if indexing
        if let Some(state) = &self.indexing_state {
            let progress_block = Block::default().borders(Borders::TOP).title(" Indexing ");

            let gauge = Gauge::default()
                .block(progress_block)
                .ratio(state.calc_progress())
                .gauge_style(Style::new().light_blue());

            frame.render_widget(gauge, main_layout[2]); // Bottom area
        }

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
            KeyCode::Char(c) => {
                // Handle command prefix for slash mode
                if self.command_style == CommandStyle::Slash
                    && c == '/'
                    && self.input_buffer.is_empty()
                {
                    self.mode = Mode::Command;
                    self.input_buffer = "/".to_string();
                } else {
                    self.input_buffer.push(c);
                }
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            _ => {}
        }
    }

    pub fn handle_command_mode(&mut self, key: KeyEvent) {
        // if !self.input_buffer.starts_with('/') {
        //     self.mode = Mode::Normal;
        // }
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                self.execute_command();
                self.input_buffer.clear();
                self.mode = Mode::Normal;
            }
            KeyCode::Char(c) => self.input_buffer.push(c),
            KeyCode::Backspace => {
                if self.input_buffer.len() == 1 && self.input_buffer.starts_with('/') {
                    self.mode = Mode::Insert;
                }
                self.input_buffer.pop();
            }
            _ => {}
        }
    }

    fn execute_command(&mut self) {
        let cmd = self.input_buffer.clone();
        // Remove command prefix for processing
        let cmd_str = match self.command_style {
            CommandStyle::NeoVim => cmd.trim_start_matches(':').trim(),
            CommandStyle::Slash => cmd.trim_start_matches('/').trim(),
        };

        match cmd_str {
            "help" => self.show_command_help(),
            "index start" => self.send_cmd(StateCommand::IndexWorkspace {
                workspace: "fixture_nodes".to_string(),
            }),
            "index pause" => self.send_cmd(StateCommand::PauseIndexing),
            "index resume" => self.send_cmd(StateCommand::ResumeIndexing),
            "index cancel" => self.send_cmd(StateCommand::CancelIndexing),
            cmd => {
                // TODO: Implement `tracing` crate import
                // Placeholder for command error handling
                eprintln!("Unknown command: {}", cmd);
            }
        }
    }

    fn show_command_help(&self) {
        // TODO: Add these as system messages.
        eprintln!("Available commands:");
        eprintln!("  index - Run workspace indexing");
        eprintln!("  help  - Show this help");
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
            KeyCode::Char(':') if self.command_style == CommandStyle::NeoVim => {
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
    kind: MessageKind,
    content: String, // Add other fields if needed for drawing, e.g. status
}

fn truncate_uuid(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;
    use std::time::Duration;

    use crate::app::{App, Mode};
    use crate::app_state::{AppState, StateCommand};
    use crate::user_config::CommandStyle;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ploke_embed::indexer::{IndexStatus, IndexingStatus};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tokio::sync::{broadcast, mpsc};
    use uuid::Uuid;

    // Helper function to create a test terminal
    fn build_test_terminal() -> Terminal<TestBackend> {
        let backend = TestBackend::new(100, 30);
        Terminal::new(backend).unwrap()
    }

    // Helper function to render terminal with delay
    fn draw_terminal_with_delay(
        terminal: &mut Terminal<TestBackend>,
        app: &App,
        _delay: Duration,
    ) -> Vec<String> {
        todo!()
    }

    #[tokio::test]
    async fn user_starts_and_monitors_indexing() {
        let (cmd_tx, mut cmd_rx) = mpsc::channel(32);
        let event_bus = Arc::new(EventBus::new(EventBusCaps {
            realtime_cap: 100,
            background_cap: 100,
            error_cap: 100,
            index_cap: 100,
        }));

        // Initialize app
        let mut app = App::new(
            CommandStyle::Slash,
            Arc::new(AppState::default()),
            cmd_tx,
            &event_bus,
        );

        // Start indexing via command
        app.mode = Mode::Command;
        app.input_buffer = "/index start fixture_nodes".into();
        app.handle_command_mode(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        // Verify command was sent
        assert!(matches!(
            cmd_rx.recv().await,
            Some(StateCommand::IndexWorkspace { .. })
        ));
    }
    //     // Simulate progress
    //     event_bus.send(AppEvent::IndexingProgress(IndexingStatus {
    //         status: IndexStatus::Running,
    //         recent_processed: 5,
    //         total: 100,
    //         current_file: None,
    //         errors: Vec::new(),
    //     }));
    //
    //     // Render
    //     let mut terminal = build_test_terminal();
    //     let frames = draw_terminal_with_delay(&mut terminal, &app, Duration::from_millis(50));
    //
    //     // Verify progress appears
    //     let frame_string = frames.join("");
    //     assert!(frame_string.contains("Indexing"));
    //     assert!(frame_string.contains("5/100"));
    // }

    // use crate::test_utils::mock::MockBehavior;
    // use mockall::{Sequence, predicate::*};
    // use ploke_embed::{
    //     error::{EmbedError, truncate_string},
    //     indexer::IndexingStatus,
    // };
    //
    // #[tokio::test]
    // async fn http_error_propagation() {
    //     // Setup
    //     let (mut progress_rx, state) = setup_test_environment(MockBehavior::RateLimited, 10).await;
    //
    //     // Capture progress state
    //     let mut status: Option<IndexingStatus> = None;
    //     while let Ok(progress) = progress_rx.recv().await {
    //         if progress.total > 0 {
    //             status = Some(progress);
    //             break;
    //         }
    //     }
    //
    //     // Add generated embeddings
    //     run_embedding_phase(&state).await;
    //
    //     // Verify error
    //     let status = status.unwrap();
    //     assert!(!status.errors.is_empty());
    //     assert!(status.errors[0].contains("429"));
    //     assert!(status.errors[0].contains("Rate Limited"));
    // }
}
