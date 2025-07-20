use crate::{app_state::ListNavigation, chat_history::MessageKind, user_config::CommandStyle};
pub mod message_item;

use super::*;
use std::time::{Duration, Instant};

use app_state::{AppState, StateCommand};
use color_eyre::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use message_item::render_messages;
use ratatui::widgets::{Gauge, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};
use textwrap::wrap;
use tracing::instrument;

static HELP_COMMANDS: &str = r#"Available commands:
    index start [directory] - Run workspace indexing on specified directory
                              (defaults to current dir)
    index pause - Pause indexing
    index resume - Resume indexing
    index cancel - Cancel indexing
    check api - Check API key configuration
    model list - List available models
    help - Show this help

    Keyboard shortcuts (Normal mode):
    q - Quit
    i - Enter insert mode
    : - Enter command mode (vim-style)
    m - Quick model selection
    ? - Show this help
    j/↓ - Navigate down
    k/↑ - Navigate up
    J - Jump to bottom
    K - Jump to top
    h/← - Navigate branch previous
    l/→ - Navigate branch next"#;

#[derive(Default, Copy, Clone, PartialEq, Eq, Debug)]
pub enum Mode {
    Normal,
    #[default]
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
    // TODO: Decide if we can get rid of this now that we have replaced this list with a custom list implementation.
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
    input_vscroll: u16,
    input_scrollstate: ScrollbarState,
    convo_vscroll: u16,
    convo_scrollstate: ScrollbarState,
    active_model_indicator: Option<(String, Instant)>,
    active_model_id: String,
    input_cursor_row: u16,
    input_cursor_col: u16,
    is_trailing_whitespace: bool,
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new(
        command_style: CommandStyle,
        state: Arc<AppState>,
        cmd_tx: mpsc::Sender<StateCommand>,
        event_bus: &EventBus, // reference non-Arc OK because only created at startup
        active_model_id: String,
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

            input_vscroll: 0,
            input_scrollstate: ScrollbarState::default(),
            convo_vscroll: 0,
            convo_scrollstate: ScrollbarState::default(),
            active_model_indicator: None,
            active_model_id,
            input_cursor_row: 0,
            input_cursor_col: 0,
            is_trailing_whitespace: false,
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

        // let mut frame_counter = 0;
        while self.running {
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
            // match self.mode {
            //     Mode::Insert | Mode::Command => {crossterm::execute!(std::io::stdout(), Show)?;},
            //     Mode::Normal => {crossterm::execute!(std::io::stdout(), Hide)?;},
            // };

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
                        AppEvent::Rag(rag_event) => {
                            // let new_msg_id = match rag_event {
                            //     RagEvent::ContextSnippets(uuid, items) => uuid,
                            //     RagEvent::UserMessages(uuid, messages) => uuid,
                            //     RagEvent::ConstructContext(uuid) => uuid,
                            // };
                            // self.send_cmd(StateCommand::ForwardContext { new_msg_id});
                        }
                        AppEvent::Error(error_event) => {
                            let msg = format!("Error: {}", error_event.message);
                            self.send_cmd(StateCommand::AddMessageImmediate {
                                msg,
                                kind: MessageKind::SysInfo,
                                new_msg_id: Uuid::new_v4(),
                            });
                        },
                        AppEvent::IndexingStarted => {
                        },
                        AppEvent::IndexingCompleted => {
                            tracing::info!("Indexing Succeeded!");
                            self.indexing_state = None;
                            self.send_cmd(StateCommand::AddMessageImmediate {
                                msg: String::from("Indexing Succeeded"),
                                kind: MessageKind::SysInfo,
                                new_msg_id: Uuid::new_v4(),
                            });
                            self.send_cmd(StateCommand::UpdateDatabase)
                        },
                        AppEvent::IndexingFailed => {
                            tracing::error!("Indexing Failed");
                            self.indexing_state = None;
                            self.send_cmd(StateCommand::AddMessageImmediate {
                                msg: String::from("Indexing Failed"),
                                kind: MessageKind::SysInfo,
                                new_msg_id: Uuid::new_v4(),
                            })
                        },
                        // AppEvent::System(system_event) => {},
                        AppEvent::System(system_event) => {
                            match system_event {
                                system::SystemEvent::ModelSwitched(new_model)=>{
                                tracing::debug!("StateCommand::ModelSwitched {}", new_model);
                                self.send_cmd(StateCommand::AddMessageImmediate {
                                    msg: format!("model changed from {} to {}",self.active_model_id, new_model),
                                    kind: MessageKind::SysInfo,
                                    new_msg_id: Uuid::new_v4(),
                                });
                                    self.active_model_indicator = Some((new_model.clone(), Instant::now()));
                                    self.active_model_id = new_model;
                                },
                                other => {tracing::warn!("Unused system event in main app loop: {:?}", other)}
                        }
                        }
                        AppEvent::GenerateContext(id) => {
                            // self.send_cmd( StateCommand::)
                        }
                    }
                }

            }
        }
        Ok(())
    }

    /// Renders the user interface.
    fn draw(&mut self, frame: &mut Frame, path: &[RenderableMessage], current_id: Uuid) {
        // Always show the currently selected model in the top-right
        let show_indicator = true;

        // ---------- Define Layout ----------
        let mut proto_layout = if self.indexing_state.is_some() {
            vec![
                Constraint::Length(1),
                Constraint::Percentage(80),
                Constraint::Percentage(20),
                Constraint::Length(1),
                Constraint::Length(3),
            ]
        } else {
            vec![
                Constraint::Length(1),
                Constraint::Percentage(80),
                Constraint::Percentage(20),
                Constraint::Length(1),
            ]
        };

        if show_indicator {
            proto_layout.push(Constraint::Length(1));
        }

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(proto_layout)
            .split(frame.area());

        let model_info_area = main_layout[0];
        let chat_area = main_layout[1];
        let input_area = main_layout[2];
        let status_area = main_layout[3];

        let status_line_area = layout_statusline(5, status_area);

        // ---------- Scroll State -------------
        // let convo_length = convo.height;
        // self.convo_scrollstate = self.convo_scrollstate.content_length(convo_length as usize);

        // ---------- Prepare Widgets ----------
        // Render message tree
        let conversation_width = main_layout[0].width.saturating_sub(6);

        render_messages(self, frame, path, conversation_width, chat_area);
        // Render input area with dynamic title
        let input_title = match (self.mode, self.command_style) {
            (Mode::Command, CommandStyle::NeoVim) => "Command Mode",
            (Mode::Command, CommandStyle::Slash) => "Slash Mode",
            _ => "Input",
        };

        // Guess at the amount of scroll needed:

        // ---------- Text Wrap ----------------
        let input_width = input_area.width.saturating_sub(2);
        let input_wrapped = textwrap::wrap(self.input_buffer.as_str(), input_width as usize);
        self.input_scrollstate = self.input_scrollstate.content_length(input_wrapped.len());

        // -- Get cursor position
        if !self.is_trailing_whitespace {
            self.input_cursor_col = input_wrapped.last().map(|line| line.len()).unwrap_or(0) as u16;
        }
        self.input_cursor_row = (input_wrapped.len().saturating_sub(1)) as u16;
        // --

        let input_text = Text::from_iter(input_wrapped);
        let input = Paragraph::new(input_text)
            .scroll((self.input_vscroll, 0))
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

            frame.render_widget(gauge, main_layout[3]); // Bottom area
        }

        // Render Mode to text
        let status_bar = Block::default()
            .title(self.mode.to_string())
            .borders(Borders::NONE)
            .padding(Padding::vertical(1));
        let node_status = Paragraph::new(format!("Node: {}", truncate_uuid(current_id)))
            .block(Block::default().borders(Borders::NONE))
            .style(Style::new().fg(Color::Blue));

        // -- Handle Scrollbars --
        // TODO: how to make this work?

        // ---------- Render widgets in layout ----------
        // -- top level
        frame.render_widget(input, input_area);
        // frame.render_stateful_widget(
        //     Scrollbar::new(ScrollbarOrientation::VerticalRight)
        //         .begin_symbol(Some("↑"))
        //         .end_symbol(Some("↓")),
        //     main_layout[1].inner(Margin {vertical: 1, horizontal: 0}),
        //     &mut self.input_scrollstate,
        // );

        // -- first nested
        frame.render_widget(status_bar, status_line_area[0]);
        frame.render_widget(node_status, status_line_area[1]);

        // -- model indicator
        if show_indicator {
            if let Some((model_name, _)) = &self.active_model_indicator {
                let indicator = Paragraph::new(format!(" Model: {} ", model_name))
                    .style(Style::new().fg(Color::White).bg(Color::DarkGray))
                    .alignment(ratatui::layout::Alignment::Right);

                frame.render_widget(indicator, model_info_area);
            }
        }

        match self.mode {
            Mode::Insert | Mode::Command => {
                // Position cursor at end of input buffer
                frame.set_cursor_position((
                    input_area.x + 1 + self.input_cursor_col,
                    input_area.y + 1 + self.input_cursor_row,
                ));
            }
            Mode::Normal => {
                // Hide cursor in normal mode
                // - By not calling `set_cursor_position`, the cursor is automatically hidden
                // NOTE: Maybe do something with the cursor in normal mode?
            }
        }
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
        if key.modifiers == KeyModifiers::CONTROL {
            match key.code {
                KeyCode::Char('a') => {
                    self.input_buffer
                        .push_str("Agnostic anthromoporcine agrippa");
                }
                // FIX: testing
                KeyCode::Up => {
                    self.input_scrollstate.prev();
                }
                KeyCode::Down => {
                    self.input_scrollstate.next();
                }
                _ => {}
            }
        }
        match key.code {
            // 1. UI-Local State Change: Switch mode
            KeyCode::Esc => self.mode = Mode::Normal,

            // 2. Shared State Change: Send a command
            KeyCode::Enter => {
                if !self.input_buffer.is_empty() && !self.input_buffer.starts_with('\n') {
                    let new_msg_id = Uuid::new_v4();
                    self.send_cmd(StateCommand::AddUserMessage {
                        // TODO: `input_buffer` doesn't need to be cloned, try to `move` it or something
                        // instead.
                        content: self.input_buffer.clone(),
                        new_msg_id,
                    });
                    // TODO: Expand EmbedMessage to include other types of message
                    self.send_cmd(StateCommand::EmbedMessage { new_msg_id });
                    self.send_cmd(StateCommand::AddMessage {
                        kind: MessageKind::SysInfo,
                        content: "Embedding User Message".to_string(),
                        target: llm::ChatHistoryTarget::Main,
                        parent_id: new_msg_id,
                        child_id: Uuid::new_v4(),
                    });
                    // self.send_cmd(StateCommand::ForwardContext { new_msg_id });
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
                    self.is_trailing_whitespace =
                        self.input_buffer.chars().last().is_some_and(|c| c == ' ');
                    if self.is_trailing_whitespace {
                        self.input_cursor_col += 1;
                    }
                }
            }
            KeyCode::Backspace => {
                let last_char = self.input_buffer.pop();
                self.input_cursor_col = self.input_cursor_col.saturating_sub(1);
                self.is_trailing_whitespace =
                    self.input_buffer.chars().last().is_some_and(|c| c == ' ');
            }
            // FIX: testing
            KeyCode::Up => {
                self.convo_scrollstate.next();
            }
            KeyCode::Down => {
                self.convo_scrollstate.prev();
            }
            _ => {}
        }
    }

    pub fn handle_command_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                self.execute_command();
                self.input_buffer.clear();
                self.mode = Mode::Insert;
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

    #[instrument(skip_all, fields(alias))]
    fn execute_command(&mut self) {
        let cmd = self.input_buffer.clone();
        // Remove command prefix for processing
        let cmd_str = match self.command_style {
            CommandStyle::NeoVim => cmd.trim_start_matches(':').trim(),
            CommandStyle::Slash => cmd.trim_start_matches('/').trim(),
        };

        match cmd_str {
            "help" => self.show_command_help(),
            cmd if cmd.starts_with("index start") => {
                let parts: Vec<&str> = cmd.splitn(3, ' ').collect();
                let workspace = if parts.len() >= 3 {
                    parts[2].to_string()
                } else {
                    // Default to current directory if no path provided
                    ".".to_string()
                };

                // Validate the directory exists
                match std::fs::metadata(&workspace) {
                    Ok(metadata) if metadata.is_dir() => {
                        self.send_cmd(StateCommand::IndexWorkspace { workspace });
                    }
                    Ok(_) => {
                        self.send_cmd(StateCommand::AddMessageImmediate {
                            msg: format!("Error: '{}' is not a directory", workspace),
                            kind: MessageKind::SysInfo,
                            new_msg_id: Uuid::new_v4(),
                        });
                    }
                    Err(e) => {
                        self.send_cmd(StateCommand::AddMessageImmediate {
                            msg: format!("Error accessing directory '{}': {}", workspace, e),
                            kind: MessageKind::SysInfo,
                            new_msg_id: Uuid::new_v4(),
                        });
                    }
                }
            }
            "index pause" => self.send_cmd(StateCommand::PauseIndexing),
            "index resume" => self.send_cmd(StateCommand::ResumeIndexing),
            "index cancel" => self.send_cmd(StateCommand::CancelIndexing),
            "check api" => {
                self.check_api_keys();
            }
            "model list" => self.list_models(),
            cmd if cmd.starts_with("model ") => {
                let alias = cmd.trim_start_matches("model ").trim();
                tracing::debug!("StateCommand::SwitchModel {}", alias);
                if !alias.is_empty() {
                    self.send_cmd(StateCommand::SwitchModel {
                        alias_or_id: alias.to_string(),
                    });
                }
            }
            cmd => {
                // TODO: Implement `tracing` crate import
                // Placeholder for command error handling
                eprintln!("Unknown command: {}", cmd);
            }
        }
    }

    fn show_command_help(&self) {
        self.send_cmd(StateCommand::AddMessageImmediate {
            msg: HELP_COMMANDS.to_string(),
            kind: MessageKind::SysInfo,
            new_msg_id: Uuid::new_v4(),
        });
    }

    /// Lists all registered provider configurations in the chat window.
    ///
    /// Reads the current provider registry from shared state (blocking only the
    /// calling thread) and emits a nicely-formatted list of available models,
    /// including both their short alias and the full model name.
    fn list_models(&self) {
        let cfg = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { self.state.config.read().await })
        });

        let mut lines = vec!["Available models:".to_string()];

        for pc in &cfg.provider_registry.providers {
            let display = pc.display_name.as_ref().unwrap_or(&pc.model);
            lines.push(format!("  {:<28}  {}", pc.id, display));
        }

        self.send_cmd(StateCommand::AddMessageImmediate {
            msg: lines.join("\n"),
            kind: MessageKind::SysInfo,
            new_msg_id: Uuid::new_v4(),
        });
    }

    fn check_api_keys(&self) {
        // This would need to be async to check the actual config
        // For now, we'll provide a helpful message
        let help_msg = r#"API Key Configuration Check:

 To use LLM features, you need to set your API keys:
 - For OpenRouter models: export OPENROUTER_API_KEY="your-key-here"
 - For OpenAI models: export OPENAI_API_KEY="your-key-here"
 - For Anthropic models: export ANTHROPIC_API_KEY="your-key-here"

 After setting the environment variable, restart the application.
 Use 'model list' to see available models."#;

        self.send_cmd(StateCommand::AddMessageImmediate {
            msg: help_msg.to_string(),
            kind: MessageKind::SysInfo,
            new_msg_id: Uuid::new_v4(),
        });
    }

    /// This function is responsible for doing something with user input when
    /// the terminal is in "Normal" Mode.
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
            KeyCode::Char('m') => {
                self.mode = Mode::Command;
                self.input_buffer = "/model ".to_string();
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
pub struct RenderableMessage {
    id: Uuid,
    kind: MessageKind,
    content: String, // Add other fields if needed for drawing, e.g. status
}

fn truncate_uuid(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}
