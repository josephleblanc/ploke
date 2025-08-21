use crate::{app_state::ListNavigation, chat_history::MessageKind, user_config::CommandStyle};
pub mod commands;
pub mod events;
pub mod input;
pub mod message_item;
pub mod types;
pub mod utils;
pub mod view;

use super::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::app::input::keymap::{Action, to_action};
use crate::app::types::{Mode, RenderMsg};
use crate::app::utils::truncate_uuid;
use crate::app::view::components::conversation::ConversationView;
use crate::app::view::components::input_box::InputView;
use app_state::{AppState, StateCommand};
use crate::llm::openrouter_catalog::ModelEntry;
use crate::user_config::{ProviderConfig, ProviderType, OPENROUTER_URL};
use color_eyre::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton,
    MouseEvent, MouseEventKind,
};
use crossterm::execute;
use itertools::Itertools;
// use message_item::{measure_messages, render_messages}; // now handled by ConversationView
use ploke_db::search_similar;
use ratatui::widgets::Gauge;
// use textwrap::wrap; // moved into InputView
use tokio::sync::oneshot;
use toml::to_string;
use tracing::instrument;

// Ensure terminal modes are always restored on unwind (panic or early return)
struct TerminalModeGuard;

impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        // Best-effort disable; ignore errors to avoid panicking in Drop
        let _ = crossterm::execute!(
            std::io::stdout(),
            DisableBracketedPaste,
            DisableFocusChange,
            DisableMouseCapture,
        );
        // ratatui::restore is called by the outer try_main panic hook as an extra safety net
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
    conversation: ConversationView,
    input_view: InputView,
    active_model_indicator: Option<(String, Instant)>,
    active_model_id: String,
    // Scrolling/UI helpers
    pending_char: Option<char>,
    needs_redraw: bool,
    show_context_preview: bool,
    // Modal overlay for interactive model discovery/selection
    model_browser: Option<ModelBrowserState>,
    // Input history browsing (Insert mode)
    input_history: Vec<String>,
    input_history_pos: Option<usize>,
}

#[derive(Debug)]
struct ModelBrowserItem {
    id: String,
    name: Option<String>,
    context_length: Option<u32>,
    input_cost: Option<f64>,
    output_cost: Option<f64>,
    supports_tools: bool,
    expanded: bool,
}

#[derive(Debug)]
struct ModelBrowserState {
    visible: bool,
    keyword: String,
    items: Vec<ModelBrowserItem>,
    selected: usize,
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

            conversation: ConversationView::default(),
            input_view: InputView::default(),
            active_model_indicator: None,
            active_model_id,
            // Scrolling/UI helpers
            pending_char: None,
            needs_redraw: true,
            show_context_preview: false,
            model_browser: None,
            input_history: Vec::new(),
            input_history_pos: None,
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
        // RAII guard to ensure terminal modes are disabled on unwind
        let _terminal_mode_guard = TerminalModeGuard;

        // Initialize the UI selection base on the initial state.
        self.sync_list_selection().await;

        // let mut frame_counter = 0;
        while self.running {
            if self.needs_redraw {
                // Prepare data for this frame by reading from AppState without allocating per-frame.
                let app_state = Arc::clone(&self.state);
                let history_guard = app_state.chat.0.read().await;
                let path_len = history_guard.path_len();
                let current_id = history_guard.current;

                // Draw the UI using iterators over the cached path.
                terminal.draw(|frame| {
                    self.draw(
                        frame,
                        history_guard.iter_path(),
                        history_guard.iter_path(),
                        path_len,
                        current_id,
                    )
                })?;
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
                                    self.conversation.scroll_lines_up(3);
                                    self.conversation.set_free_scrolling(true);
                                    self.pending_char = None;
                                    self.needs_redraw = true;
                                }
                                MouseEventKind::ScrollDown => {
                                    self.conversation.scroll_lines_down(3);
                                    self.conversation.set_free_scrolling(true);
                                    self.pending_char = None;
                                    self.needs_redraw = true;
                                }
                                MouseEventKind::Down(MouseButton::Left) => {
                                    // Hit-test inside chat area to select message on click
                                    let area = self.conversation.last_chat_area();
                                    let x = mouse_event.column;
                                    let y = mouse_event.row;
                                    if x >= area.x
                                        && x < area.x.saturating_add(area.width)
                                        && y >= area.y
                                        && y < area.y.saturating_add(area.height)
                                    {
                                        let rel_y = y.saturating_sub(area.y);
                                        let virtual_line = self.conversation.offset().saturating_add(rel_y);

                                        let mut acc = 0u16;
                                        let mut target_idx_opt: Option<usize> = None;
                                        for (i, h) in self.conversation.item_heights().iter().enumerate() {
                                            let next_acc = acc.saturating_add(*h);
                                            if virtual_line < next_acc {
                                                target_idx_opt = Some(i);
                                                break;
                                            }
                                            acc = next_acc;
                                        }
                                        let len = self.conversation.item_heights().len();
                                        if len > 0 {
                                            let target_idx = target_idx_opt.unwrap_or_else(|| len.saturating_sub(1));

                                            // Update UI selection immediately
                                            let prev_sel = self.list.selected();
                                            self.list.select(Some(target_idx));
                                            self.conversation.set_free_scrolling(false);
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
                events::handle_event(&mut self, app_event).await;
                self.needs_redraw = true;
            }

            }
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
    fn draw<'a, I1, I2, T: RenderMsg + 'a>(
        &mut self,
        frame: &mut Frame,
        path_for_measure: I1,
        path_for_render: I2,
        path_len: usize,
        current_id: Uuid,
    ) where
        I1: IntoIterator<Item = &'a T>,
        I2: IntoIterator<Item = &'a T>,
    {
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
        let chat_area_full = main_layout[1];
        let input_area = main_layout[2];
        let status_area = main_layout[3];

        // Optionally split chat into conversation (left) and context preview (right)
        let (chat_area, preview_area_opt) = if self.show_context_preview {
            let chat_columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                .split(chat_area_full);
            (chat_columns[0], Some(chat_columns[1]))
        } else {
            (chat_area_full, None)
        };

        // Remember conversation area for mouse hit-testing is handled by ConversationView.

        let status_line_area = layout_statusline(5, status_area);

        // ---------- Prepare Widgets ----------
        // Render message tree
        let conversation_width = chat_area.width.saturating_sub(6);
        let viewport_height = chat_area.height;

        // Clamp selected index to valid range to avoid OOB when the path shrinks between frames.
        let selected_index_opt = self
            .list
            .selected()
            .map(|i| i.min(path_len.saturating_sub(1)));

        // Prepare and render conversation via ConversationView
        self.conversation.prepare(
            path_for_measure,
            path_len,
            conversation_width,
            viewport_height,
            selected_index_opt,
        );
        self.conversation.set_last_chat_area(chat_area);
        self.conversation.render(
            frame,
            path_for_render,
            conversation_width,
            chat_area,
            selected_index_opt,
        );

        // Right-side context preview (placeholder until wired to Rag events)
        if let Some(preview_area) = preview_area_opt {
            let preview = Paragraph::new("Context Preview\nWaiting for results…")
                .block(Block::bordered().title(" Context Preview "));
            frame.render_widget(preview, preview_area);
        }

        // Render input area with dynamic title
        let input_title = match (self.mode, self.command_style) {
            (Mode::Command, CommandStyle::NeoVim) => "Command Mode",
            (Mode::Command, CommandStyle::Slash) => "Slash Mode",
            _ => "Input",
        };

        // Render input box via InputView
        self.input_view.render(
            frame,
            input_area,
            &self.input_buffer,
            if self.model_browser.is_some() { Mode::Normal } else { self.mode },
            input_title,
        );
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
        // InputView rendered above.
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

        // Render model browser overlay if visible
        if let Some(mb) = &self.model_browser {
            let area = frame.area();
            let width = area.width.saturating_mul(8) / 10;
            let height = area.height.saturating_mul(8) / 10;
            let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
            let y = area.y.saturating_add(area.height.saturating_sub(height) / 2);
            let rect = ratatui::layout::Rect::new(x, y, width.max(40), height.max(10));

            let mut lines: Vec<String> = Vec::new();
            lines.push(format!(
                "Model Browser — {} results for \"{}\"",
                mb.items.len(),
                mb.keyword
            ));
            lines.push("Instructions: ↑/↓ or j/k to navigate, Enter/Space to expand, s to select, q/Esc to close.".to_string());
            lines.push(String::new());

            for (i, it) in mb.items.iter().enumerate() {
                let sel = if i == mb.selected { ">" } else { " " };
                let title = if let Some(name) = &it.name {
                    if name.is_empty() {
                        it.id.clone()
                    } else {
                        format!("{} — {}", it.id, name)
                    }
                } else {
                    it.id.clone()
                };
                lines.push(format!("{} {}", sel, title));
                if it.expanded {
                    lines.push(format!(
                        "    context_length: {}",
                        it.context_length
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "-".to_string())
                    ));
                    lines.push(format!("    supports_tools: {}", it.supports_tools));
                    lines.push(format!(
                        "    pricing: in={} out={}",
                        it.input_cost
                            .map(|v| format!("{:.4}", v))
                            .unwrap_or_else(|| "-".to_string()),
                        it.output_cost
                            .map(|v| format!("{:.4}", v))
                            .unwrap_or_else(|| "-".to_string())
                    ));
                }
            }

            let content = lines.join("\n");
            let widget = Paragraph::new(content)
                .block(Block::bordered().title(" Model Browser "))
                .wrap(ratatui::widgets::Wrap { trim: true });
            frame.render_widget(widget, rect);
        }

        // Cursor position is handled by InputView.
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

    /// Handles the key events and updates application state via high-level Actions.
    ///
    /// Phase 1 refactor: convert KeyEvent -> Action in input::keymap, then handle here.
    fn on_key_event(&mut self, key: KeyEvent) {
        // Intercept keys for model browser overlay when visible
        if self.model_browser.is_some() {
            let mut chosen_id: Option<String> = None;
            if let Some(mb) = self.model_browser.as_mut() {
                use KeyCode::*;
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        self.model_browser = None;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if mb.selected > 0 {
                            mb.selected -= 1;
                        } else {
                            mb.selected = mb.items.len().saturating_sub(1);
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if mb.items.is_empty() {
                            // nothing
                        } else if mb.selected + 1 < mb.items.len() {
                            mb.selected += 1;
                        } else {
                            mb.selected = 0;
                        }
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        if let Some(item) = mb.items.get_mut(mb.selected) {
                            item.expanded = !item.expanded;
                        }
                    }
                    KeyCode::Char('s') => {
                        chosen_id = mb.items.get(mb.selected).map(|i| i.id.clone());
                    }
                    _ => {}
                }
            }
            // Drop the mutable borrow of self.model_browser before switching model
            if let Some(id) = chosen_id {
                self.switch_to_model(&id);
                self.model_browser = None;
            }
            self.needs_redraw = true;
            return;
        }

        // Insert mode input history navigation
        if self.mode == Mode::Insert {
            use KeyCode::*;
            match key.code {
                KeyCode::Up => {
                    self.input_history_prev();
                    self.needs_redraw = true;
                    return;
                }
                KeyCode::Down => {
                    self.input_history_next();
                    self.needs_redraw = true;
                    return;
                }
                KeyCode::PageUp => {
                    self.input_history_first();
                    self.needs_redraw = true;
                    return;
                }
                KeyCode::PageDown => {
                    self.input_history_last();
                    self.needs_redraw = true;
                    return;
                }
                _ => {}
            }
        } else {
            // Normal mode: delete the currently selected message with Del
            if matches!(key.code, crossterm::event::KeyCode::Delete) {
                let id = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        let guard = self.state.chat.0.read().await;
                        guard.current
                    })
                });
                self.send_cmd(StateCommand::DeleteMessage { id });
                self.needs_redraw = true;
                return;
            }
        }

        if let Some(action) = to_action(self.mode, key, self.command_style) {
            self.handle_action(action);
        }
        self.needs_redraw = true;
    }

    /// Centralized Action handler. This consolidates the previous per-mode handlers
    /// into a single, testable entrypoint.
    fn handle_action(&mut self, action: Action) {
        use crate::chat_history::NavigationDirection::{Next, Previous};

        match action {
            Action::Quit => {
                self.quit();
            }

            Action::SwitchMode(new_mode) => {
                self.mode = new_mode;
                self.pending_char = None;
            }

            Action::InsertChar(c) => {
                // While typing, keep the viewport stable (disable auto-centering on selection)
                self.conversation.set_free_scrolling(true);
                // Special-case: Slash style treats leading '/' as entering Command mode.
                if self.mode == Mode::Insert
                    && self.command_style == CommandStyle::Slash
                    && c == '/'
                    && self.input_buffer.is_empty()
                {
                    self.mode = Mode::Command;
                    self.input_buffer = "/".to_string();
                } else {
                    self.add_input_char(c);
                }
            }

            Action::Backspace => {
                if self.mode == Mode::Command
                    && self.input_buffer.len() == 1
                    && self.input_buffer.starts_with('/')
                {
                    self.mode = Mode::Insert;
                }
                // While editing, avoid auto-scrolling caused by selection adjustments
                self.conversation.set_free_scrolling(true);
                self.handle_backspace();
            }

            Action::Submit => {
                // Enter in Insert mode: send the user's message via StateCommands.
                if !self.input_buffer.is_empty() && !self.input_buffer.starts_with('\n') {
                    let (completion_tx, completion_rx) = oneshot::channel();
                    let (scan_tx, scan_rx) = oneshot::channel();
                    let new_msg_id = Uuid::new_v4();
                    self.send_cmd(StateCommand::AddUserMessage {
                        content: self.input_buffer.clone(),
                        new_msg_id,
                        completion_tx,
                    });
                    self.send_cmd(StateCommand::ScanForChange { scan_tx });
                    self.send_cmd(StateCommand::EmbedMessage {
                        new_msg_id,
                        completion_rx,
                        scan_rx,
                    });
                    self.send_cmd(StateCommand::AddMessage {
                        kind: MessageKind::SysInfo,
                        content: "Embedding User Message".to_string(),
                        target: llm::ChatHistoryTarget::Main,
                        parent_id: new_msg_id,
                        child_id: Uuid::new_v4(),
                    });
                    // Snap to bottom to ensure the full assistant/system response is visible.
                    self.conversation.request_bottom();
                    self.conversation.set_free_scrolling(true);
                    self.input_buffer.clear();
                }
            }

            Action::ExecuteCommand => {
                self.execute_command();
                // Ensure snap-to-bottom so long outputs (e.g., /help) are fully visible.
                self.conversation.request_bottom();
                self.conversation.set_free_scrolling(true);
                self.input_buffer.clear();
                self.mode = Mode::Insert;
            }

            Action::NavigateListUp => {
                self.conversation.set_free_scrolling(false);
                self.pending_char = None;
                self.send_cmd(StateCommand::NavigateList {
                    direction: ListNavigation::Up,
                });
            }
            Action::NavigateListDown => {
                self.conversation.set_free_scrolling(false);
                self.pending_char = None;
                self.send_cmd(StateCommand::NavigateList {
                    direction: ListNavigation::Down,
                });
            }

            Action::PageDown => {
                self.conversation.page_down();
                self.conversation.set_free_scrolling(true);
                self.pending_char = None;
            }
            Action::PageUp => {
                self.conversation.page_up();
                self.conversation.set_free_scrolling(true);
                self.pending_char = None;
            }

            Action::BranchPrev => {
                self.conversation.set_free_scrolling(false);
                self.pending_char = None;
                self.send_cmd(StateCommand::NavigateBranch {
                    direction: Previous,
                });
            }
            Action::BranchNext => {
                self.conversation.set_free_scrolling(false);
                self.pending_char = None;
                self.send_cmd(StateCommand::NavigateBranch { direction: Next });
            }

            Action::ScrollLineDown => {
                self.conversation.scroll_line_down();
                self.conversation.set_free_scrolling(true);
                self.pending_char = None;
            }
            Action::ScrollLineUp => {
                self.conversation.scroll_line_up();
                self.conversation.set_free_scrolling(true);
                self.pending_char = None;
            }

            Action::GotoSequenceG => {
                if matches!(self.pending_char, Some('g')) {
                    // gg -> bottom (preserve existing behavior)
                    self.send_cmd(StateCommand::NavigateList {
                        direction: ListNavigation::Top,
                    });
                    self.conversation.request_bottom();
                    self.conversation.set_free_scrolling(false);
                    self.pending_char = None;
                } else {
                    self.pending_char = Some('g');
                }
            }
            Action::JumpTop => {
                // 'G' -> top (preserve existing behavior)
                self.send_cmd(StateCommand::NavigateList {
                    direction: ListNavigation::Bottom,
                });
                self.conversation.request_top();
                self.conversation.set_free_scrolling(false);
                self.pending_char = None;
            }

            Action::OpenCommand => {
                self.pending_char = None;
                self.mode = Mode::Command;
                if self.command_style == CommandStyle::Slash {
                    self.input_buffer = "/hybrid ".to_string();
                } else {
                    self.input_buffer = ":hybrid ".to_string();
                }
            }
            Action::OpenCommandColon => {
                self.pending_char = None;
                self.mode = Mode::Command;
                self.input_buffer = ":".to_string();
            }
            Action::OpenQuickModel => {
                self.pending_char = None;
                self.mode = Mode::Command;
                self.input_buffer = "/model ".to_string();
            }
            Action::OpenHelp => {
                self.pending_char = None;
                self.mode = Mode::Command;
                self.input_buffer = "/help".to_string();
            }
            Action::TogglePreview => {
                self.pending_char = None;
                self.show_context_preview = !self.show_context_preview;
            }

            Action::InputScrollPrev => {
                self.input_view.scroll_prev();
            }
            Action::InputScrollNext => {
                self.input_view.scroll_next();
            }
        }
    }

    fn handle_backspace(&mut self) {
        let _ = self.input_buffer.pop();
    }

    fn add_input_char(&mut self, c: char) {
        // Typing resets input-history browsing
        self.input_history_pos = None;
        self.input_buffer.push(c);
    }

    /// Rebuild the per-conversation user-input history from the current path.
    fn rebuild_input_history(&mut self) {
        let msgs = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let guard = self.state.chat.0.read().await;
                guard
                    .get_full_path()
                    .into_iter()
                    .filter(|m| m.kind == MessageKind::User && !m.content.is_empty())
                    .map(|m| m.content.clone())
                    .collect::<Vec<String>>()
            })
        });
        self.input_history = msgs;
    }

    fn input_history_prev(&mut self) {
        if self.input_history.is_empty() {
            self.rebuild_input_history();
        }
        if self.input_history.is_empty() {
            return;
        }
        match self.input_history_pos {
            None => {
                // Start from most recent (last)
                let last = self.input_history.len().saturating_sub(1);
                self.input_history_pos = Some(last);
                self.input_buffer = self.input_history[last].clone();
            }
            Some(pos) => {
                if pos > 0 {
                    let new_pos = pos - 1;
                    self.input_history_pos = Some(new_pos);
                    self.input_buffer = self.input_history[new_pos].clone();
                }
            }
        }
    }

    fn input_history_next(&mut self) {
        if self.input_history.is_empty() {
            self.rebuild_input_history();
        }
        if self.input_history.is_empty() {
            return;
        }
        match self.input_history_pos {
            None => {
                // Nothing selected; keep buffer as-is
            }
            Some(pos) => {
                if pos + 1 < self.input_history.len() {
                    let new_pos = pos + 1;
                    self.input_history_pos = Some(new_pos);
                    self.input_buffer = self.input_history[new_pos].clone();
                } else {
                    // Beyond the newest -> clear buffer and exit history mode
                    self.input_history_pos = None;
                    self.input_buffer.clear();
                }
            }
        }
    }

    fn input_history_first(&mut self) {
        if self.input_history.is_empty() {
            self.rebuild_input_history();
        }
        if self.input_history.is_empty() {
            return;
        }
        self.input_history_pos = Some(0);
        self.input_buffer = self.input_history[0].clone();
    }

    fn input_history_last(&mut self) {
        if self.input_history.is_empty() {
            self.rebuild_input_history();
        }
        if self.input_history.is_empty() {
            return;
        }
        let last = self.input_history.len().saturating_sub(1);
        self.input_history_pos = Some(last);
        self.input_buffer = self.input_history[last].clone();
    }

    fn open_model_browser(&mut self, keyword: String, items: Vec<ModelEntry>) {
        let items = items
            .into_iter()
            .map(|m| ModelBrowserItem {
                id: m.id,
                name: m.name,
                context_length: m.context_length,
                input_cost: m.pricing.as_ref().and_then(|p| p.input),
                output_cost: m.pricing.as_ref().and_then(|p| p.output),
                supports_tools: m.capabilities.as_ref().and_then(|c| c.tools).unwrap_or(false),
                expanded: false,
            })
            .collect::<Vec<_>>();

        self.model_browser = Some(ModelBrowserState {
            visible: true,
            keyword,
            selected: 0,
            items,
        });
        self.needs_redraw = true;
    }

    fn close_model_browser(&mut self) {
        self.model_browser = None;
        self.needs_redraw = true;
    }

    fn switch_to_model(&mut self, model_id: &str) {
        // Mutate runtime config to promote or select the model, then broadcast info
        let state = Arc::clone(&self.state);
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let mut cfg = state.config.write().await;
                let reg = &mut cfg.provider_registry;

                let exists = reg.providers.iter().any(|p| p.id == model_id);
                if !exists {
                    // Promote discovered model into registry
                    reg.providers.push(ProviderConfig {
                        id: model_id.to_string(),
                        api_key: String::new(),
                        api_key_env: Some("OPENROUTER_API_KEY".to_string()),
                        base_url: OPENROUTER_URL.to_string(),
                        model: model_id.to_string(),
                        display_name: Some(model_id.to_string()),
                        provider_type: ProviderType::OpenRouter,
                        llm_params: Some(crate::llm::LLMParameters {
                            model: model_id.to_string(),
                            ..Default::default()
                        }),
                    });
                    // Load keys across providers
                    reg.load_api_keys();
                }

                // Switch active provider
                reg.active_provider = model_id.to_string();
            });
        });

        self.active_model_id = model_id.to_string();
        self.active_model_indicator = Some((self.active_model_id.clone(), Instant::now()));
        self.send_cmd(StateCommand::AddMessageImmediate {
            msg: format!("Switched active model to {}", model_id),
            kind: MessageKind::SysInfo,
            new_msg_id: Uuid::new_v4(),
        });
        self.needs_redraw = true;
    }

    fn execute_command(&mut self) {
        commands::execute_command(self);
    }

    fn show_command_help(&self) {
        self.send_cmd(StateCommand::AddMessageImmediate {
            msg: commands::HELP_COMMANDS.to_string(),
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

    /// Set running to false to quit the application.
    fn quit(&mut self) {
        self.running = false;
    }
}
