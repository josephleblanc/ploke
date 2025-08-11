use crate::{app_state::ListNavigation, chat_history::MessageKind, user_config::CommandStyle};
pub mod message_item;

use super::*;
use std::time::{Duration, Instant};

use app_state::{AppState, StateCommand};
use color_eyre::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton,
    MouseEvent, MouseEventKind,
};
use message_item::{measure_messages, render_messages};
use ploke_db::search_similar;
use ratatui::widgets::{Gauge, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};
use textwrap::wrap;
use tokio::sync::oneshot;
use toml::to_string;
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
    j/↓ - Navigate down (selection)
    k/↑ - Navigate up (selection)
    J - Page down (scroll)
    K - Page up (scroll)
    G - Go to bottom (scroll)
    gg - Go to top (scroll)
    h/← - Navigate branch previous
    l/→ - Navigate branch next
    Ctrl+n - Scroll down one line
    Ctrl+p - Scroll up one line"#;

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
    // Conversation viewport scrolling state
    convo_offset_y: u16,
    convo_content_height: u16,
    convo_item_heights: Vec<u16>,
    convo_auto_follow: bool,
    active_model_indicator: Option<(String, Instant)>,
    active_model_id: String,
    input_cursor_row: u16,
    input_cursor_col: u16,
    is_trailing_whitespace: bool,
    // Scrolling/UI helpers
    convo_free_scrolling: bool,
    pending_char: Option<char>,
    last_viewport_height: u16,
    last_chat_area: ratatui::layout::Rect,
    needs_redraw: bool,
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
            // Conversation viewport scrolling init
            convo_offset_y: 0,
            convo_content_height: 0,
            convo_item_heights: Vec::new(),
            convo_auto_follow: false,
            active_model_indicator: None,
            active_model_id,
            input_cursor_row: 0,
            input_cursor_col: 0,
            is_trailing_whitespace: false,
            convo_free_scrolling: false,
            pending_char: None,
            last_viewport_height: 0,
            last_chat_area: ratatui::layout::Rect::default(),
            needs_redraw: true,
        }
    }

    fn send_cmd(&self, cmd: StateCommand) {
        // Use try_send to prevent the UI from blocking
        if let Err(e) = self.cmd_tx.try_send(cmd) {
            tracing::warn!("Failed to send command: {}", e);
        }
    }

    /// Run the application's main loop.
    pub async fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.running = true;
        let mut crossterm_events = crossterm::event::EventStream::new();
        if let Err(e) = execute!(
            std::io::stdout(),
            EnableBracketedPaste,
            EnableFocusChange,
            EnableMouseCapture
        ) {
            tracing::warn!("Failed to enable terminal modes: {}", e);
        }

        // Initialize the UI selection base on the initial state.
        self.sync_list_selection().await;

        // let mut frame_counter = 0;
        while self.running {
            if self.needs_redraw {
                // Prepare data for this frame by reading from AppState.
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

                // Draw the UI with the prepared data.
                terminal.draw(|frame| self.draw(frame, &renderable_messages, current_id))?;
                self.needs_redraw = false;
            }

            // Handle all incoming events (user input, state changes).
            tokio::select! {
            // Prioritize Ui responsiveness
            biased;

            // User input
            maybe_event = crossterm_events.next().fuse() => {
                if let Some(Ok(event)) = maybe_event {
                    match event {
                        Event::Key(key_event) =>{ self.on_key_event(key_event); self.needs_redraw = true; }
                        Event::FocusGained => {},
                        Event::FocusLost => {},
                        Event::Mouse(mouse_event) => {
                            match mouse_event.kind {
                                MouseEventKind::ScrollUp => {
                                    // Free scroll up by 3 lines, clamp at 0
                                    self.convo_offset_y = self.convo_offset_y.saturating_sub(3);
                                    self.convo_free_scrolling = true;
                                    self.pending_char = None;
                                    self.needs_redraw = true;
                                }
                                MouseEventKind::ScrollDown => {
                                    // Free scroll down by 3 lines, clamp to max offset
                                    let max_offset = self
                                        .convo_content_height
                                        .saturating_sub(self.last_viewport_height);
                                    let new_offset = self.convo_offset_y.saturating_add(3);
                                    self.convo_offset_y = new_offset.min(max_offset);
                                    self.convo_free_scrolling = true;
                                    self.pending_char = None;
                                    self.needs_redraw = true;
                                }
                                MouseEventKind::Down(MouseButton::Left) => {
                                    // Hit-test inside chat area to select message on click
                                    let area = self.last_chat_area;
                                    let x = mouse_event.column;
                                    let y = mouse_event.row;
                                    if x >= area.x
                                        && x < area.x.saturating_add(area.width)
                                        && y >= area.y
                                        && y < area.y.saturating_add(area.height)
                                    {
                                        let rel_y = y.saturating_sub(area.y);
                                        let virtual_line = self.convo_offset_y.saturating_add(rel_y);

                                        let mut acc = 0u16;
                                        let mut target_idx_opt: Option<usize> = None;
                                        for (i, h) in self.convo_item_heights.iter().enumerate() {
                                            let next_acc = acc.saturating_add(*h);
                                            if virtual_line < next_acc {
                                                target_idx_opt = Some(i);
                                                break;
                                            }
                                            acc = next_acc;
                                        }
                                        let len = self.convo_item_heights.len();
                                        if len > 0 {
                                            let target_idx = target_idx_opt.unwrap_or_else(|| len.saturating_sub(1));

                                            // Update UI selection immediately
                                            let prev_sel = self.list.selected();
                                            self.list.select(Some(target_idx));
                                            self.convo_free_scrolling = false;
                                            self.pending_char = None;

                                            // Sync AppState selection using existing navigation commands
                                            match prev_sel {
                                                Some(prev) if target_idx > prev => {
                                                    for _ in 0..(target_idx - prev) {
                                                        self.send_cmd(StateCommand::NavigateList {
                                                            direction: ListNavigation::Down,
                                                        });
                                                    }
                                                }
                                                 Some(prev) if prev > target_idx => {
                                                    for _ in 0..(prev - target_idx) {
                                                        self.send_cmd(StateCommand::NavigateList {
                                                            direction: ListNavigation::Up,
                                                        });
                                                    }
                                                }
                                                // do nothing if selecting the current item.
                                                Some(_) => {},
                                                None => {
                                                    // Choose shortest path via Top/Bottom
                                                    if target_idx < len / 2 {
                                                        self.send_cmd(StateCommand::NavigateList {
                                                            direction: ListNavigation::Top,
                                                        });
                                                        for _ in 0..target_idx {
                                                            self.send_cmd(StateCommand::NavigateList {
                                                                direction: ListNavigation::Down,
                                                            });
                                                        }
                                                    } else {
                                                        self.send_cmd(StateCommand::NavigateList {
                                                            direction: ListNavigation::Bottom,
                                                        });
                                                        for _ in 0..(len.saturating_sub(1).saturating_sub(target_idx)) {
                                                            self.send_cmd(StateCommand::NavigateList {
                                                                direction: ListNavigation::Up,
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    self.needs_redraw = true;
                                }
                                _ => {}
                            }
                        },
                        Event::Paste(_) => {},
                        Event::Resize(_, _) => { self.needs_redraw = true; },
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
                    AppEvent::Rag(rag_event) => {},
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
                    // NOTE: This system event handling is a bad pattern. This should probably be
                    // managed by the event_bus system instead.
                    AppEvent::System(system_event) => {
                        match system_event {
                            system::SystemEvent::ModelSwitched(new_model)=>{
                                tracing::debug!("SystemEvent::ModelSwitched {}", new_model);
                                self.send_cmd(StateCommand::AddMessageImmediate {
                                    msg: format!("model changed from {} to {}",self.active_model_id, new_model),
                                    kind: MessageKind::SysInfo,
                                    new_msg_id: Uuid::new_v4(),
                                });
                                self.active_model_indicator = Some((new_model.clone(), Instant::now()));
                                self.active_model_id = new_model;
                            },
                            SystemEvent::ReadQuery{ file_name, query_name } => {
                                tracing::debug!("App receives event: {}", file_name);
                                self.send_cmd(StateCommand::AddMessageImmediate {
                                    msg: format!("Reading file for query called {query_name}:\n\t{file_name}"),
                                    kind: MessageKind::SysInfo,
                                    new_msg_id: Uuid::new_v4(),
                                });

                            },
                            SystemEvent::LoadQuery{ query_name, query_content } => {
                                tracing::debug!("App receives LoadQuery from FileManager for {query_name}:\n{query_content}");
                                let shortened_query = query_content.chars().take(20).collect::<String>();
                                self.send_cmd(StateCommand::AddMessageImmediate {
                                    msg: format!("Query read from file with query name {query_name}:\n\t{shortened_query}..."),
                                    kind: MessageKind::SysInfo,
                                    new_msg_id: Uuid::new_v4(),
                                });
                                self.send_cmd(StateCommand::LoadQuery {
                                    query_name,
                                    query_content,
                                });
                            },
                            SystemEvent::BackupDb {file_dir, is_success, .. } if is_success => {
                                // TODO: Add crate name to data type and require in command
                                tracing::debug!("App receives BackupDb successful db save to file: {}", &file_dir);
                                    self.send_cmd(StateCommand::AddMessageImmediate {
                                        msg: format!("Success: Cozo data for code graph saved successfully to {file_dir}"),
                                        kind: MessageKind::SysInfo,
                                        new_msg_id: Uuid::new_v4(),
                                    });

                            },
                            SystemEvent::BackupDb {file_dir, is_success, error } if !is_success => {
                                // TODO: Add crate name to data type and require in command
                                tracing::debug!("App receives BackupDb unsuccessful event: {}\nwith error: {:?}", &file_dir, &error);
                                    if let Some(error_str) = error {
                                        self.send_cmd(StateCommand::AddMessageImmediate {
                                            msg: format!("Error: Cozo data for code graph not saved to {file_dir}\n\tFailed with error: {}", &error_str),
                                            kind: MessageKind::SysInfo,
                                            new_msg_id: Uuid::new_v4(),
                                        });
                                    }
                            },
                            SystemEvent::LoadDb {crate_name, file_dir, is_success, .. } if is_success => {
                                tracing::debug!("App receives LoadDb successful db save to file: {:?}", 
                                    display_file_info(file_dir.as_ref()), 
                                );
                                self.send_cmd(StateCommand::AddMessageImmediate {
                                    msg: format!("Success: Cozo data for code graph loaded successfully for {crate_name} from {}", 
                                        display_file_info(file_dir.as_ref()), 
                                    ),
                                    kind: MessageKind::SysInfo,
                                    new_msg_id: Uuid::new_v4(),
                                });
                            },
                            SystemEvent::LoadDb {crate_name, file_dir, is_success, error } if !is_success => {
                                // TODO: Add crate name to data type and require in command
                                tracing::debug!("App receives LoadDb unsuccessful event: {}\nwith error: {:?}", 
                                    display_file_info(file_dir.as_ref()), 
                                    &error
                                );
                                if let Some(error_str) = error {
                                    self.send_cmd(StateCommand::AddMessageImmediate {
                                        msg: format!("Error: Cozo data for code graph of {crate_name} not loaded from {}\n\tFailed with error: {}", 
                                            display_file_info(file_dir.as_ref()), 
                                            &error_str),
                                        kind: MessageKind::SysInfo,
                                        new_msg_id: Uuid::new_v4(),
                                    });
                                }
                            },
                            SystemEvent::ReIndex { workspace } => {
                                    self.send_cmd(StateCommand::IndexWorkspace { workspace, needs_parse: false });
                                }
                            other => {tracing::warn!("Unused system event in main app loop: {:?}", other)}
                        }
                    }
                    AppEvent::GenerateContext(id) => {
                        // self.send_cmd( StateCommand::)
                    }
                }
                self.needs_redraw = true;
            }

            }
        }

        fn display_file_info(file: Option<&Arc<std::path::PathBuf>>) -> String {
            file.map(|f| f.display().to_string()).unwrap_or("File not found.".to_string())
        }
        if let Err(e) = execute!(
            std::io::stdout(),
            DisableBracketedPaste,
            DisableFocusChange,
            DisableMouseCapture
        ) {
            tracing::warn!("Failed to disable terminal modes: {}", e);
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
        // Remember chat area for mouse hit-testing
        self.last_chat_area = chat_area;

        let status_line_area = layout_statusline(5, status_area);

        // ---------- Prepare Widgets ----------
        // Render message tree
        let conversation_width = chat_area.width.saturating_sub(6);
        let viewport_height = chat_area.height;
        self.last_viewport_height = viewport_height;

        // 1) Measure current frame (no rendering)
        // Clamp selected index to valid range to avoid OOB when the path shrinks between frames.
        let selected_index_opt = self
            .list
            .selected()
            .map(|i| i.min(path.len().saturating_sub(1)));
        let (total_height, heights) = measure_messages(path, conversation_width, selected_index_opt);

        // 2) Decide/adjust offset using current metrics
        let max_offset = total_height.saturating_sub(viewport_height);

        if path.is_empty() {
            // Nothing to render; keep viewport at top and mark as following.
            self.convo_offset_y = 0;
            self.convo_auto_follow = true;
            self.convo_free_scrolling = false;
        } else if let Some(selected_index) = selected_index_opt {
            let is_last = selected_index + 1 == path.len();
            if is_last {
                // Only force bottom if auto-follow is enabled and user is not free-scrolling.
                if self.convo_auto_follow && !self.convo_free_scrolling {
                    self.convo_offset_y = max_offset;
                }
            } else if !self.convo_free_scrolling {
                // Exit auto-follow when navigating to a non-last message and minimally reveal selection
                self.convo_auto_follow = false;

                // Minimally reveal selection within current viewport
                let mut prefix_sum = 0u16;
                for (i, h) in heights.iter().enumerate() {
                    if i == selected_index {
                        break;
                    }
                    prefix_sum = prefix_sum.saturating_add(*h);
                }
                let selected_top = prefix_sum;
                let selected_bottom = prefix_sum.saturating_add(heights[selected_index]);
                let viewport_bottom = self.convo_offset_y.saturating_add(viewport_height);

                if selected_top < self.convo_offset_y {
                    self.convo_offset_y = selected_top;
                } else if selected_bottom > viewport_bottom {
                    self.convo_offset_y = selected_bottom.saturating_sub(viewport_height);
                }
            }
        } else {
            // No explicit selection; keep existing offset (will clamp below)
        }

        // Clamp offset to valid range
        if self.convo_offset_y > max_offset {
            self.convo_offset_y = max_offset;
        }

        // 3) Persist metrics and auto-follow status
        self.convo_content_height = total_height;
        self.convo_item_heights = heights;
        if !self.convo_free_scrolling {
            if let Some(selected_index) = selected_index_opt {
                let is_last = selected_index + 1 == path.len();
                self.convo_auto_follow = is_last || self.convo_offset_y >= max_offset;
            } else {
                self.convo_auto_follow = self.convo_offset_y >= max_offset;
            }
        }

        // 4) Render with final offset
        render_messages(
            frame,
            path,
            conversation_width,
            chat_area,
            self.convo_offset_y,
            &self.convo_item_heights,
            selected_index_opt,
        );

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

            frame.render_widget(gauge, main_layout[4]); // Bottom area
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
        //     input_area.inner(Margin {vertical: 1, horizontal: 0}),
        //     &mut self.input_scrollstate,
        // );

        // -- first nested
        frame.render_widget(status_bar, status_line_area[0]);
        frame.render_widget(node_status, status_line_area[1]);

        // -- model indicator (always visible)
        let display_model = self
            .active_model_id
            .split("/")
            .last()
            .unwrap_or(&self.active_model_id);
        log::debug!("display_model: {}", display_model);

        let model_display = Paragraph::new(format!(" {} ", display_model))
            .style(Style::new().fg(Color::Green))
            .alignment(ratatui::layout::Alignment::Right);
        frame.render_widget(model_display, model_info_area);

        // Flash indicator for model changes
        if let Some((_, timestamp)) = &self.active_model_indicator {
            if timestamp.elapsed().as_secs() < 2 {
                let flash_indicator = Paragraph::new("✓");
                frame.render_widget(
                    flash_indicator,
                    ratatui::layout::Rect::new(
                        model_info_area.x.saturating_sub(2),
                        model_info_area.y,
                        2,
                        1,
                    ),
                );
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
                // NOTE: This is here just for testing, remove it when we actually want to release
                // this.
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
                    // Somewhat complex implementation here, could use some work.
                    // - Basically, we first start adding the user message, which is then updated
                    // after we have embedded the user message.
                    // - The currently selected crate is then parsed, checking to see if we need to
                    // update the database or not. Currently this is quite coarse, such that we
                    // reparse the entire directory if any file changes are noticed. However, we
                    // only update the embeddings of the changed files.
                    // - Concurrently with the parsing, the user's message is embedded, then once
                    // the oneshot is sent to signify that the parsing has finished and database
                    // has been updated (if needed), then the user's message is used with semantic
                    // search to query the database, and continues into context building and
                    // finally sending the message to the LLM.
                    let (completion_tx, completion_rx) = oneshot::channel();
                    let (scan_tx, scan_rx) = oneshot::channel();
                    let new_msg_id = Uuid::new_v4();
                    self.send_cmd(StateCommand::AddUserMessage {
                        // TODO: `input_buffer` doesn't need to be cloned, try to `move` it or something
                        // instead.
                        content: self.input_buffer.clone(),
                        new_msg_id,
                        completion_tx,
                    });
                    self.send_cmd(StateCommand::ScanForChange { scan_tx });
                    // TODO: Expand EmbedMessage to include other types of message
                    self.send_cmd(StateCommand::EmbedMessage {
                        new_msg_id,
                        completion_rx,
                        scan_rx
                    });
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
                    self.add_input_char(c);
                }
            }
            KeyCode::Backspace => self.handle_backspace(),
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

    fn handle_backspace(&mut self) {
        let last_char = self.input_buffer.pop();
        self.input_cursor_col = self.input_cursor_col.saturating_sub(1);
        self.is_trailing_whitespace = self.input_buffer.chars().last().is_some_and(|c| c == ' ');
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
            KeyCode::Char(c) => self.add_input_char(c),
            KeyCode::Backspace => {
                if self.input_buffer.len() == 1 && self.input_buffer.starts_with('/') {
                    self.mode = Mode::Insert;
                }
                self.handle_backspace();
            }
            _ => {}
        }
    }

    fn add_input_char(&mut self, c: char) {
        self.input_buffer.push(c);
        self.is_trailing_whitespace = self.input_buffer.chars().last().is_some_and(|c| c == ' ');
        if self.is_trailing_whitespace {
            self.input_cursor_col += 1;
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
            // TODO: Add an indicator that the command is recognized before the user enters the
            // command. This is one cool benefit of having an immediate mode renderer.
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
                        self.send_cmd(StateCommand::IndexWorkspace { workspace, needs_parse: true });
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
            "save history" => {
                self.send_cmd(StateCommand::AddMessageImmediate {
                    msg: "Saving conversation history...".to_string(),
                    kind: MessageKind::SysInfo,
                    new_msg_id: Uuid::new_v4(),
                });
                self.send_cmd(StateCommand::SaveState);
            }
            // Loads a single target backup database from the default config dir into cozo,
            // overwriting any currently loaded db.
            // Expects a the command `/load crate`
            cmd if cmd.starts_with("load crate") => {
                match cmd.trim_start_matches("load crate").trim() {
                    crate_name if !crate_name.contains(' ') => {
                        self.send_cmd(StateCommand::AddMessageImmediate {
                            msg: format!("Attempting to load code graph for {crate_name}..."),
                            kind: MessageKind::SysInfo,
                            new_msg_id: Uuid::new_v4(),
                        });
                        self.send_cmd(StateCommand::LoadDb {
                            crate_name: crate_name.to_string(),
                        });
                    }
                    _ => {
                        self.send_cmd(StateCommand::AddMessageImmediate {
                        msg: "Please enter the name of the crate you wish to load.\nThe crates with db backups are located in your default config directory.".to_string(),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                        });
                    }
                }
            }
            "query load" | "ql" => {
                self.send_cmd(StateCommand::ReadQuery {
                    query_name: "default".to_string(),
                    file_name: "default.dl".to_string(),
                });
            }
            "save db" | "sd" => {
                self.send_cmd(StateCommand::SaveDb);
            }
            cmd if cmd.starts_with("query load ") => {
                if let Some((query_name, file_name)) =
                    cmd.trim_start_matches("query load ").trim().split_once(' ')
                {
                    tracing::debug!("Reading Query {} from file {}", query_name, file_name);
                    self.send_cmd(StateCommand::ReadQuery {
                        query_name: query_name.to_string(),
                        file_name: file_name.to_string(),
                    });
                }
            }
            cmd if cmd.starts_with("batch") => {
                let mut parts = cmd.split_whitespace();
                parts.next(); // skip "batch"
                let prompt_file = parts.next().unwrap_or("queries.txt");
                let out_file = parts.next().unwrap_or("results.json");
                self.send_cmd(StateCommand::BatchPromptSearch {
                    prompt_file: prompt_file.to_string(),
                    out_file: out_file.to_string(),
                    max_hits: None,
                    threshold: None,
                });
            }
            cmd => {
                // TODO: Implement `tracing` crate import
                // Placeholder for command error handling
                // Add more helpful message here
                self.show_command_help();
                tracing::warn!("Unknown command: {}", cmd);
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

        // Free-scrolling controls (Normal mode) with Ctrl modifiers
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('n') => {
                    // Line down
                    self.convo_offset_y = self.convo_offset_y.saturating_add(1);
                    self.convo_free_scrolling = true;
                    self.pending_char = None;
                }
                KeyCode::Char('p') => {
                    // Line up
                    self.convo_offset_y = self.convo_offset_y.saturating_sub(1);
                    self.convo_free_scrolling = true;
                    self.pending_char = None;
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.quit(),

            // --- NAVIGATION ---
            // Send commands instead of calling local methods
            KeyCode::Char('k') | KeyCode::Up => {
                self.convo_free_scrolling = false;
                self.pending_char = None;
                self.send_cmd(StateCommand::NavigateList {
                    direction: ListNavigation::Up,
                });
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.convo_free_scrolling = false;
                self.pending_char = None;
                self.send_cmd(StateCommand::NavigateList {
                    direction: ListNavigation::Down,
                });
            }
            // Page scrolling with Shift-J / Shift-K
            KeyCode::Char('J') => {
                let vh = self.last_viewport_height.max(1);
                let page_step: u16 = (vh / 10).clamp(1, 5);
                self.convo_offset_y = self.convo_offset_y.saturating_add(page_step);
                self.convo_free_scrolling = true;
                self.pending_char = None;
            }
            KeyCode::Char('K') => {
                let vh = self.last_viewport_height.max(1);
                let page_step: u16 = (vh / 10).clamp(1, 5);
                self.convo_offset_y = self.convo_offset_y.saturating_sub(page_step);
                self.convo_free_scrolling = true;
                self.pending_char = None;
            }
            // Branch navigation clears free-scrolling to allow reveal
            KeyCode::Char('h') | KeyCode::Left => {
                self.convo_free_scrolling = false;
                self.pending_char = None;
                self.send_cmd(StateCommand::NavigateBranch {
                    direction: Previous,
                });
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.convo_free_scrolling = false;
                self.pending_char = None;
                self.send_cmd(StateCommand::NavigateBranch { direction: Next });
            }

            // Jump to bottom/top and select message
            KeyCode::Char('g') => {
                if matches!(self.pending_char, Some('g')) {
                    // gg -> bottom: select last message and scroll to bottom
                    self.send_cmd(StateCommand::NavigateList {
                        direction: ListNavigation::Top,
                    });
                    self.convo_offset_y = u16::MAX; // will clamp to bottom on draw
                    self.convo_free_scrolling = false;
                    self.pending_char = None;
                } else {
                    // wait for second 'g'
                    self.pending_char = Some('g');
                }
            }
            KeyCode::Char('G') => {
                // Top: select first message and scroll to top
                self.send_cmd(StateCommand::NavigateList {
                    direction: ListNavigation::Bottom,
                });
                self.convo_offset_y = 0;
                self.convo_free_scrolling = false;
                self.pending_char = None;
            }

            // --- COMMANDS ---
            KeyCode::Char(':') if self.command_style == CommandStyle::NeoVim => {
                self.pending_char = None;
                self.mode = Mode::Command;
                self.input_buffer = ":".to_string();
            }
            KeyCode::Char('m') => {
                self.pending_char = None;
                self.mode = Mode::Command;
                self.input_buffer = "/model ".to_string();
            }
            KeyCode::Char('?') => {
                self.pending_char = None;
                self.mode = Mode::Command;
                self.input_buffer = "/help".to_string();
            }
            KeyCode::Char('i') => {
                self.pending_char = None;
                self.mode = Mode::Insert;
            }
            _ => {
                // Clear any pending multi-key sequence
                self.pending_char = None;
            }
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
