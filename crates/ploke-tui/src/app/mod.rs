use crate::llm::provider_endpoints::SupportsTools as _;
use crate::{app_state::ListNavigation, chat_history::MessageKind, user_config::CommandStyle};
pub mod commands;
pub mod editor;
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
use crate::emit_app_event;
use crate::llm::openrouter_catalog::ModelEntry;
use crate::user_config::{OPENROUTER_URL, ModelConfig, ProviderType};
use app_state::{AppState, StateCommand};
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
use ratatui::text::{Line, Span};
use ratatui::widgets::Gauge;
// use textwrap::wrap; // moved into InputView
use tokio::sync::oneshot;
use toml::to_string;
use tracing::instrument;
use view::components::model_browser::{render_model_browser, ModelBrowserItem, ModelBrowserState, ModelProviderRow};
use crate::app::editor::{resolve_editor_command, build_editor_args};
use view::components::approvals::{render_approvals_overlay, ApprovalsState};

// Ensure terminal modes are always restored on unwind (panic or early return)
struct TerminalModeGuard {
    enabled: bool,
}

impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        if self.enabled {
            // Best-effort disable; ignore errors to avoid panicking in Drop
            let _ = crossterm::execute!(
                std::io::stdout(),
                DisableBracketedPaste,
                DisableFocusChange,
                DisableMouseCapture,
            );
        }
        // ratatui::restore is called by the outer try_main panic hook as an extra safety net
    }
}

/// Options controlling how the TUI run loop configures the terminal.
/// In tests, prefer `setup_terminal_modes: false` to avoid taking over the host terminal.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunOptions {
    pub setup_terminal_modes: bool,
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
    // Modal overlay for approvals list
    approvals: Option<ApprovalsState>,
    // Input history browsing (Insert mode)
    input_history: Vec<String>,
    input_history_pos: Option<usize>,
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
            approvals: None,
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

    /// Run the application's main loop with a generic backend and input stream.
    /// Use `run` for the default production path; use this for tests with `TestBackend`.
    pub async fn run_with<B, S>(
        mut self,
        mut terminal: ratatui::Terminal<B>,
        mut input: S,
        opts: RunOptions,
    ) -> Result<()>
    where
        B: ratatui::backend::Backend,
        S: futures::Stream<Item = std::result::Result<crossterm::event::Event, std::io::Error>> + Unpin,
    {
        use futures::StreamExt;
        self.running = true;
        if opts.setup_terminal_modes {
            if let Err(e) = execute!(
                std::io::stdout(),
                EnableBracketedPaste,
                EnableFocusChange,
                EnableMouseCapture
            ) {
                tracing::warn!("Failed to enable terminal modes: {}", e);
            }
        }
        // RAII guard to ensure terminal modes are disabled on unwind
        let _terminal_mode_guard = TerminalModeGuard { enabled: opts.setup_terminal_modes };

        // Initialize the UI selection base on the initial state.
        self.sync_list_selection().await;

        // If the provided input stream ends (e.g., tests using an empty stream),
        // stop polling it to avoid starving event handling.
        let mut input_done = false;

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

            // User input (only while input stream is active)
            maybe_event = input.next().fuse(), if !input_done => {
                match maybe_event {
                    Some(Ok(event)) => {
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
                    // Stream ended or error: stop polling input to avoid busy-loop
                    _ => { input_done = true; }
                }
            }

            // Application events
            Ok(app_event) = self.event_rx.recv() => {
                events::handle_event(&mut self, app_event).await;
                self.needs_redraw = true;
            }

            }
        }

        // Terminal modes are disabled by TerminalModeGuard when enabled
        Ok(())
    }

    /// Run the application's main loop with the default terminal backend and real input events.
    pub async fn run(self, terminal: DefaultTerminal) -> Result<()> {
        use futures::StreamExt;
        let crossterm_events = crossterm::event::EventStream::new();
        self
            .run_with(terminal, crossterm_events, RunOptions { setup_terminal_modes: true })
            .await
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
            if self.model_browser.is_some() {
                Mode::Normal
            } else {
                self.mode
            },
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
            let (body_area, footer_area, overlay_style, lines) = render_model_browser(frame, mb);

            let widget = Paragraph::new(lines)
                .style(overlay_style)
                .block(
                    Block::bordered()
                        .title(" Model Browser ")
                        .style(overlay_style),
                )
                // Preserve leading indentation in detail lines
                .wrap(ratatui::widgets::Wrap { trim: false });
            frame.render_widget(widget, body_area);

            // Footer: bottom-right help toggle or expanded help
            if mb.help_visible {
                let help = Paragraph::new(
                    "Keys: s=select  Enter/Space=toggle details  j/k,↑/↓=navigate  q/Esc=close\n\
                     Save/Load/Search:\n\
                     - model save [path] [--with-keys]\n\
                     - model load [path]\n\
                     - model search <keyword>",
                )
                .style(overlay_style)
                .block(Block::bordered().title(" Help ").style(overlay_style))
                .wrap(ratatui::widgets::Wrap { trim: true });
                frame.render_widget(help, footer_area);
            } else {
                let hint = Paragraph::new(" ? Help ")
                    .style(overlay_style)
                    .alignment(ratatui::layout::Alignment::Right)
                    .block(Block::default().style(overlay_style));
                frame.render_widget(hint, footer_area);
            }
        }

        // Render approvals overlay if visible (on top)
        if let Some(approvals) = &self.approvals {
            // Centered overlay
            let w = frame.area().width.saturating_mul(8) / 10;
            let h = frame.area().height.saturating_mul(8) / 10;
            let x = frame.area().x + (frame.area().width.saturating_sub(w)) / 2;
            let y = frame.area().y + (frame.area().height.saturating_sub(h)) / 2;
            let overlay_area = ratatui::layout::Rect::new(x, y, w, h);
            let _ = render_approvals_overlay(frame, overlay_area, &self.state, approvals);
        }

        // Cursor position is handled by InputView.
    }

    fn handle_overlay_key(&mut self, key: KeyEvent) -> bool {
        use crossterm::event::KeyCode;
        if self.approvals.is_none() { return false; }
        let mut close = false;
        let mut approve = false;
        let mut deny = false;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => { close = true; }
            KeyCode::Up | KeyCode::Char('k') => { if let Some(st) = &mut self.approvals { st.select_prev(); } }
            KeyCode::Down | KeyCode::Char('j') => { if let Some(st) = &mut self.approvals { st.select_next(); } }
            KeyCode::Enter | KeyCode::Char('y') => { approve = true; }
            KeyCode::Char('n') | KeyCode::Char('d') => { deny = true; }
            KeyCode::Char('?') => {
                if let Some(st) = &mut self.approvals {
                    st.help_visible = !st.help_visible;
                }
                return true;
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                if let Some(st) = &mut self.approvals {
                    st.increase_view_lines();
                }
                return true;
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                if let Some(st) = &mut self.approvals {
                    st.decrease_view_lines();
                }
                return true;
            }
            KeyCode::Char('u') => {
                if let Some(st) = &mut self.approvals {
                    st.toggle_unlimited();
                }
                return true;
            }
            KeyCode::Char('o') => {
                // Open-in-editor for the first file of selected proposal
                if let Some(st) = &self.approvals {
                    let sel_index = st.selected;
                    let state = Arc::clone(&self.state);
                    let cmd_tx = self.cmd_tx.clone();
                    tokio::spawn(async move {
                        let guard = state.proposals.read().await;
                        let mut ids: Vec<uuid::Uuid> = guard.keys().cloned().collect();
                        ids.sort();
                        if let Some(id) = ids.get(sel_index) {
                            if let Some(p) = guard.get(id) {
                                if let Some(path) = p.files.first() {
                                    let cfg = state.config.read().await;
                                    let editor = resolve_editor_command(&cfg);
                                    drop(cfg);
                                    if let Some(cmd) = editor {
                                        let args = build_editor_args(path, None);
                                        let _ = std::process::Command::new(cmd).args(args).spawn();
                                    } else {
                                        let _ = cmd_tx.try_send(StateCommand::AddMessageImmediate { msg: "No editor configured. Set PLOKE_EDITOR or config ploke_editor.".into(), kind: MessageKind::SysInfo, new_msg_id: uuid::Uuid::new_v4() });
                                    }
                                }
                            }
                        }
                    });
                }
                return true;
            }
            _ => {}
        }
        if close { self.approvals = None; return true; }
        if approve || deny {
            if let Some(st) = &self.approvals {
                let sel_index = st.selected;
                let state = Arc::clone(&self.state);
                let cmd_tx = self.cmd_tx.clone();
                tokio::spawn(async move {
                    let guard = state.proposals.read().await;
                    let mut ids: Vec<uuid::Uuid> = guard.keys().cloned().collect();
                    ids.sort();
                    if let Some(id) = ids.get(sel_index) {
                        let _ = if approve {
                            cmd_tx.try_send(StateCommand::ApproveEdits { request_id: *id })
                        } else {
                            cmd_tx.try_send(StateCommand::DenyEdits { request_id: *id })
                        };
                    }
                });
            }
            return true;
        }
        true
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
        // Intercept approvals overlay keys
        if self.approvals.is_some() && self.handle_overlay_key(key) { return; }
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
                            // On expand, if providers not yet loaded, request endpoints
                            if item.expanded && item.providers.is_empty() && !item.loading_providers {
                                item.loading_providers = true;
                                let model_id = item.id.clone();
                                tokio::spawn(async move {
                                    crate::emit_app_event(crate::AppEvent::ModelEndpointsRequest { model_id }).await;
                                });
                            }
                        }
                    }
                    KeyCode::Char('s') => {
                        if let Some(item) = mb.items.get_mut(mb.selected) {
                            if item.providers.is_empty() {
                                // Fetch endpoints first, then auto-select when results arrive
                                if !item.loading_providers {
                                    item.loading_providers = true;
                                    item.pending_select = true;
                                    let model_id = item.id.clone();
                                    tokio::spawn(async move {
                                        crate::emit_app_event(crate::AppEvent::ModelEndpointsRequest { model_id }).await;
                                    });
                                } else {
                                    // Already loading; just mark pending select
                                    item.pending_select = true;
                                }
                            } else {
                                // Choose a provider that supports tools if available, otherwise first provider
                                let provider_choice = item
                                    .providers
                                    .iter()
                                    .find(|p| p.supports_tools)
                                    .or_else(|| item.providers.first())
                                    .map(|p| p.id.clone());
                                if let Some(pid) = provider_choice {
                                    chosen_id = Some(format!("{}::{}", item.id, pid));
                                } else {
                                    chosen_id = Some(item.id.clone());
                                }
                            }
                        }
                    }
                    KeyCode::Char('?') => {
                        mb.help_visible = !mb.help_visible;
                    }
                    _ => {}
                }
            }
            // Drop the mutable borrow of self.model_browser before switching model
            if let Some(id) = chosen_id {
                // id format: "model_id::provider_id" when provider selected, or just model_id
                if let Some((model_id, provider_id)) = id.split_once("::") {
                    self.apply_model_provider_selection(model_id, Some(provider_id));
                } else {
                    self.apply_model_provider_selection(&id, None);
                }
                self.model_browser = None;
            }
            self.needs_redraw = true;
            return;
        }

        // Global action mapping (including OpenApprovals)
        if let Some(action) = to_action(self.mode, key, self.command_style) {
            use Action::*;
            if let OpenApprovals = action {
                if self.approvals.is_some() { self.approvals = None; } else { self.approvals = Some(ApprovalsState::default()); }
                self.needs_redraw = true;
                return;
            }
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
                // Use node-only deletion semantics (re-parent children)
                self.send_cmd(StateCommand::DeleteNode { id });
                self.needs_redraw = true;
                return;
            }
        }

        if let Some(action) = to_action(self.mode, key, self.command_style) {
            self.handle_action(action);
        }
        self.needs_redraw = true;
    }

    fn apply_model_provider_selection(&mut self, model_id: &str, provider_slug: Option<&str>) {
        // Delegate persistence and broadcasts to the state manager (non-blocking for UI)
        let provider_id = provider_slug.unwrap_or("-");
        self.send_cmd(StateCommand::SelectModelProvider {
            model_id: model_id.to_string(),
            provider_id: provider_id.to_string(),
        });
        self.needs_redraw = true;
    }

    /// Centralized Action handler. This consolidates the previous per-mode handlers
    /// into a single, testable entrypoint.
    fn handle_action(&mut self, action: Action) {
        use crate::chat_history::NavigationDirection::{Next, Previous};

        match action {
            Action::OpenApprovals => {
                if self.approvals.is_some() {
                    self.approvals = None;
                } else {
                    self.approvals = Some(ApprovalsState::default());
                }
            }
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
            .map(|m| {
                // Provider rows
                let provider_rows = m
                    .providers
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| ModelProviderRow {
                        id: p.id,
                        context_length: p.context_length,
                        input_cost: p.pricing.as_ref().map(|pr| pr.prompt),
                        output_cost: p.pricing.as_ref().map(|p| p.completion),
                        supports_tools: p
                            .supported_parameters
                            .as_ref()
                            .map(|v| v.iter().any(|s| s.eq_ignore_ascii_case("tools")))
                            .or_else(|| p.capabilities.as_ref().and_then(|c| c.tools))
                            .unwrap_or(false),
                    })
                    .collect::<Vec<_>>();

                // Model-level tools: true if any provider supports tools OR model supported_parameters says so
                let model_supports_tools = m
                    .supported_parameters
                    .as_ref()
                    .map(|v| v.supports_tools())
                    .unwrap_or(false)
                    || provider_rows.iter().any(|p| p.supports_tools);

                ModelBrowserItem {
                    id: m.id,
                    name: m.name,
                    context_length: m
                        .context_length
                        .or_else(|| m.top_provider.as_ref().and_then(|tp| tp.context_length)),
                    input_cost: m.pricing.as_ref().map(|p| p.prompt),
                    output_cost: m.pricing.as_ref().map(|p| p.completion),
                    supports_tools: model_supports_tools,
                    providers: provider_rows,
                    expanded: false,
                    loading_providers: false,
                    pending_select: false,
                }
            })
            .collect::<Vec<_>>();

        self.model_browser = Some(ModelBrowserState {
            visible: true,
            keyword,
            selected: 0,
            items,
            help_visible: false,
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
                let reg = &mut cfg.model_registry;

                let exists = reg.providers.iter().any(|p| p.id == model_id);
                if !exists {
                    // Promote discovered model into registry
                    reg.providers.push(ModelConfig {
                        id: model_id.to_string(),
                        api_key: String::new(),
                        provider_slug: None,
                        api_key_env: Some("OPENROUTER_API_KEY".to_string()),
                        base_url: OPENROUTER_URL.to_string(),
                        model: model_id.to_string(),
                        display_name: Some(model_id.to_string()),
                        provider_type: ProviderType::OpenRouter,
                        llm_params: Some(crate::llm::LLMParameters {
                            model: model_id.to_string(),
                            // AI: Maybe we should add a field here to require models with tools?
                            ..Default::default()
                        }),
                    });
                    // Load keys across providers
                    reg.load_api_keys();
                }

                // Switch active provider
                reg.active_model_config = model_id.to_string();
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

        for pc in &cfg.model_registry.providers {
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

    pub fn set_selected_model(&mut self, model_id: String) {
        self.active_model_id = model_id;
    }

    // Test-only helpers to exercise overlay and key handling without exposing internals publicly
    /// Open the approvals overlay (intended for tests and scripted UI flows)
    pub fn approvals_open(&mut self) {
        self.approvals = Some(ApprovalsState::default());
    }

    /// Close the approvals overlay (intended for tests and scripted UI flows)
    pub fn approvals_close(&mut self) {
        self.approvals = None;
    }

    /// Inject a KeyEvent into the App input handler (intended for tests)
    pub fn push_test_key(&mut self, key: KeyEvent) {
        self.on_key_event(key);
    }

    // Test-only accessor to shared AppState for integration tests via test_harness
    pub(crate) fn test_get_state(&self) -> Arc<AppState> {
        Arc::clone(&self.state)
    }
}
