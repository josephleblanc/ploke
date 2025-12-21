use crate::app::view::components::context_browser::{
    self, ContextSearchState, SearchItem, render_context_search,
};
use crate::llm::request::models;
use crate::llm::router_only::RouterVariants;
use crate::llm::router_only::openrouter::OpenRouter;
use crate::llm::{EndpointKey, LlmEvent, ModelId, ModelKey, ModelVariant, ProviderKey};
use crate::{app_state::ListNavigation, chat_history::MessageKind, user_config::CommandStyle};
use ploke_llm::manager::events::endpoint;
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
use crate::tools::ToolVerbosity;
use crate::app::view::components::input_box::InputView;
use crate::emit_app_event;
use crate::user_config::OPENROUTER_URL;
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
use ploke_core::rag_types::ContextPart;
// use message_item::{measure_messages, render_messages}; // now handled by ConversationView
use ploke_db::search_similar;
use ratatui::text::{Line, Span};
use ratatui::widgets::Gauge;
// use textwrap::wrap; // moved into InputView
use crate::app::editor::{build_editor_args, resolve_editor_command};
use tokio::sync::oneshot;
use tokio::time::Instant as TokioInstant;
use toml::to_string;
use tracing::instrument;
use view::components::approvals::{
    ApprovalListItem, ApprovalsFilter, ApprovalsState, ProposalKind, filtered_items,
    render_approvals_overlay,
};
use view::components::embedding_browser::{
    EmbeddingBrowserItem, EmbeddingBrowserState, EmbeddingDetail, compute_embedding_browser_scroll,
    render_embedding_browser,
};
use view::components::model_browser::{
    ModelBrowserItem, ModelBrowserState, ModelProviderRow, compute_browser_scroll,
    model_browser_focus_line, model_browser_total_lines, render_model_browser,
};

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
    /// A channel to receive real-time broadcasted application events.
    event_rx: tokio::sync::broadcast::Receiver<AppEvent>,
    /// A channel to receive background-priority broadcasted application events.
    bg_event_rx: tokio::sync::broadcast::Receiver<AppEvent>,
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
    // Modal overlay for embedding model discovery/selection
    embedding_browser: Option<EmbeddingBrowserState>,
    // Context Search overlay for interactive exploration of code context
    context_browser: Option<ContextSearchState>,
    // Modal overlay for approvals list
    approvals: Option<ApprovalsState>,
    // Input history browsing (Insert mode)
    input_history: Vec<String>,
    input_history_pos: Option<usize>,
    tool_verbosity: ToolVerbosity,
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
            bg_event_rx: event_bus.subscribe(EventPriority::Background),
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
            embedding_browser: None,
            approvals: None,
            input_history: Vec::new(),
            input_history_pos: None,
            context_browser: None,
            tool_verbosity: ToolVerbosity::Normal,
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
        S: futures::Stream<Item = std::result::Result<crossterm::event::Event, std::io::Error>>
            + Unpin,
    {
        use futures::StreamExt;
        self.running = true;
        #[allow(clippy::collapsible_if)]
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
        let _terminal_mode_guard = TerminalModeGuard {
            enabled: opts.setup_terminal_modes,
        };

        // Initialize the UI selection base on the initial state.
        self.sync_list_selection().await;

        // If the provided input stream ends (e.g., tests using an empty stream),
        // stop polling it to avoid starving event handling.
        let mut input_done = false;

        // Light tick for overlays that need debounce without touching global UI cadence.
        let context_tick = tokio::time::sleep(Duration::from_millis(30));
        tokio::pin!(context_tick);

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

            // Application events (realtime)
            Ok(app_event_rt) = self.event_rx.recv() => {
                events::handle_event(&mut self, app_event_rt).await;
                self.needs_redraw = true;
            }

            // Application events (background)
            Ok(app_event_bg) = self.bg_event_rx.recv() => {
                events::handle_event(&mut self, app_event_bg).await;
                self.needs_redraw = true;
            }

            // Debounced overlay ticks (context browser)
            _ = &mut context_tick, if self.context_browser_needs_tick() => {
                self.tick_context_browser();
                context_tick.as_mut().reset(TokioInstant::now() + Duration::from_millis(30));
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
        self.run_with(
            terminal,
            crossterm_events,
            RunOptions {
                setup_terminal_modes: true,
            },
        )
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
            .map(|i: usize| -> usize { i.min(path_len.saturating_sub(1)) });

        // Prepare and render conversation via ConversationView
        self.conversation.prepare(
            path_for_measure,
            path_len,
            conversation_width,
            viewport_height,
            selected_index_opt,
            self.tool_verbosity,
        );
        self.conversation.set_last_chat_area(chat_area);
        self.conversation.render(
            frame,
            path_for_render,
            conversation_width,
            chat_area,
            selected_index_opt,
            self.tool_verbosity,
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

        let model_display = Paragraph::new(format!(" {} ", display_model))
            .style(Style::new().fg(Color::Green))
            .alignment(ratatui::layout::Alignment::Right);
        frame.render_widget(model_display, model_info_area);

        // Flash indicator for model changes
        if let Some((_, timestamp)) = &self.active_model_indicator
            && timestamp.elapsed().as_secs() < 2
        {
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

        // Render model browser overlay if visible
        if let Some(mb) = &mut self.model_browser {
            let (body_area, footer_area, overlay_style, lines) = render_model_browser(frame, mb);

            // Keep focused row visible and clamp vscroll
            compute_browser_scroll(body_area, mb);

            let widget = Paragraph::new(lines)
                .style(overlay_style)
                .block(
                    Block::bordered()
                        .title(format!(
                            " Model Browser — {} results for \"{}\" ",
                            mb.items.len(),
                            mb.keyword
                        ))
                        .style(overlay_style),
                )
                // Preserve leading indentation in detail lines
                .wrap(ratatui::widgets::Wrap { trim: false })
                .scroll((mb.vscroll, 0));
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
        } else if let Some(eb) = &mut self.embedding_browser {
            let (body_area, footer_area, overlay_style, lines) =
                render_embedding_browser(frame, eb);

            compute_embedding_browser_scroll(body_area, eb);

            let widget = Paragraph::new(lines)
                .style(overlay_style)
                .block(
                    Block::bordered()
                        .title(format!(
                            " Embedding Models — {} results for \"{}\" ",
                            eb.items.len(),
                            eb.keyword
                        ))
                        .style(overlay_style),
                )
                .wrap(ratatui::widgets::Wrap { trim: false })
                .scroll((eb.vscroll, 0));
            frame.render_widget(widget, body_area);

            if eb.help_visible {
                let help = Paragraph::new(
                    "Keys: s=select  Enter/Space=toggle details  j/k,↑/↓=navigate  q/Esc=close\n\
                     Command:\n\
                     - embedding search <keyword>",
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
        } else if let Some(cb) = &mut self.context_browser {
            let (body_area, footer_area, overlay_style, lines) = render_context_search(frame, cb);

            let free_width = body_area.width.saturating_sub(43) as usize;
            let trunc_search_string: String = cb.input.as_str().chars().take(free_width).collect();

            // WARNING: temporarily taking this line out due to borrowing issues, need to turn it
            // on for better functionality later
            // user_search::compute_browser_scroll(body_area, cb);

            // subtract 43 for the length of the surrounding text in the `format!` call for the
            // widget title below.
            let widget = Paragraph::new(lines)
                .style(overlay_style)
                .block(
                    Block::bordered()
                        .title(format!(
                            " Context Browser — {} results for \"{}\" ",
                            cb.items.len(),
                            trunc_search_string
                        ))
                        .style(overlay_style),
                )
                // Preserve leading indentation in detail lines
                .wrap(ratatui::widgets::Wrap { trim: false })
                .scroll((cb.vscroll, 0));
            frame.render_widget(widget, body_area);
            if cb.help_visible {
                // NOTE: placeholder for now, not actually functional
                let help = Paragraph::new(
                    "Keys: Enter/Space=toggle details  j/k,↑/↓=navigate  q/Esc=close\n\
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
        if self.approvals.is_none() {
            return false;
        }
        let mut close = false;
        let mut approve = false;
        let mut deny = false;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                close = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(st) = &mut self.approvals {
                    st.select_prev();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(st) = &mut self.approvals {
                    st.select_next();
                }
            }
            KeyCode::Enter | KeyCode::Char('y') => {
                approve = true;
            }
            KeyCode::Char('n') | KeyCode::Char('d') => {
                deny = true;
            }
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
            KeyCode::Char('f') => {
                if let Some(st) = &mut self.approvals {
                    st.cycle_filter();
                }
                return true;
            }
            KeyCode::Char('o') => {
                // Open-in-editor for the first file of selected proposal (edit or create)
                if let Some(st) = &self.approvals {
                    let sel_index = st.selected;
                    let filter = st.filter;
                    let state = Arc::clone(&self.state);
                    let cmd_tx = self.cmd_tx.clone();
                    tokio::spawn(async move {
                        // Build unified ordering to match overlay
                        let items = filtered_items(&state, filter);
                        if let Some(ApprovalListItem { kind, id, .. }) =
                            items.get(sel_index).cloned()
                        {
                            let path_opt = match kind {
                                ProposalKind::Edit => {
                                    let guard = state.proposals.read().await;
                                    guard.get(&id).and_then(|p| p.files.first().cloned())
                                }
                                ProposalKind::Create => {
                                    let guard = state.create_proposals.read().await;
                                    guard.get(&id).and_then(|p| p.files.first().cloned())
                                }
                            };
                            if let Some(path) = path_opt {
                                let cfg = state.config.read().await;
                                let editor = resolve_editor_command(&cfg);
                                drop(cfg);
                                if let Some(cmd) = editor {
                                    let args = build_editor_args(&path, None);
                                    let _ = std::process::Command::new(cmd).args(args).spawn();
                                } else {
                                    let _ = cmd_tx.try_send(StateCommand::AddMessageImmediate { msg: "No editor configured. Set PLOKE_EDITOR or config ploke_editor.".into(), kind: MessageKind::SysInfo, new_msg_id: uuid::Uuid::new_v4() });
                                }
                            }
                        }
                    });
                }
                return true;
            }
            _ => {}
        }
        if close {
            self.approvals = None;
            return true;
        }
        if approve || deny {
            if let Some(st) = &self.approvals {
                let sel_index = st.selected;
                let filter = st.filter;
                let state = Arc::clone(&self.state);
                let cmd_tx = self.cmd_tx.clone();
                tokio::spawn(async move {
                    // Build unified item list asynchronously to avoid blocking UI thread
                    let items = filtered_items(&state, filter);
                    if let Some(ApprovalListItem { kind, id, .. }) = items.get(sel_index).cloned() {
                        let _ = match (approve, kind) {
                            (true, ProposalKind::Edit) => {
                                cmd_tx.try_send(StateCommand::ApproveEdits { request_id: id })
                            }
                            (true, ProposalKind::Create) => {
                                cmd_tx.try_send(StateCommand::ApproveCreations { request_id: id })
                            }
                            (false, ProposalKind::Edit) => {
                                cmd_tx.try_send(StateCommand::DenyEdits { request_id: id })
                            }
                            (false, ProposalKind::Create) => {
                                cmd_tx.try_send(StateCommand::DenyCreations { request_id: id })
                            }
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
    }

    /// Handles the key events and updates application state via high-level Actions.
    ///
    /// Phase 1 refactor: convert KeyEvent -> Action in input::keymap, then handle here.
    fn on_key_event(&mut self, key: KeyEvent) {
        // Intercept approvals overlay keys
        if self.approvals.is_some() && self.handle_overlay_key(key) {
            return;
        }
        // Intercept keys for model browser overlay when visible
        if self.model_browser.is_some() {
            input::model_browser::handle_model_browser_input(self, key);
            self.needs_redraw = true;
            return;
        } else if self.embedding_browser.is_some() {
            input::embedding_browser::handle_embedding_browser_input(self, key);
            self.needs_redraw = true;
            return;
        // Intercept keys for context browser overlay when visible
        } else if self.context_browser.is_some() {
            input::context_browser::handle_context_browser_input(self, key);
            self.needs_redraw = true;
            return;
        }

        // Global action mapping (including OpenApprovals)
        if let Some(action) = to_action(self.mode, key, self.command_style)
            && Action::OpenApprovals == action
        {
            if self.approvals.is_some() {
                self.approvals = None;
            } else {
                self.approvals = Some(ApprovalsState::default());
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

    fn apply_model_provider_selection(
        &mut self,
        // keeps this string because we need to look it up in the registry from user input.
        model_id_string: String,
        provider_key: Option<ProviderKey>,
    ) {
        // Delegate persistence and broadcasts to the state manager (non-blocking for UI)
        self.send_cmd(StateCommand::SelectModelProvider {
            model_id_string,
            provider_key,
        });
        self.needs_redraw = true;
    }

    fn apply_embedding_model_selection(&mut self, model_id: ModelId, provider: Option<ArcStr>) {
        self.send_cmd(StateCommand::AddMessageImmediate {
            msg: format!("Selected embedding model {model_id}"),
            kind: MessageKind::SysInfo,
            new_msg_id: Uuid::new_v4(),
        });
        self.send_cmd(StateCommand::SelectEmbeddingModel {
            model_id,
            provider: provider.unwrap_or(ArcStr::from("openrouter")),
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
                    let new_user_msg_id = Uuid::new_v4();
                    let next_llm_msg_id = Uuid::new_v4();
                    // TODO: Add new event with user + llm message ids to co-ordinate how they are
                    // received in the llm loop
                    self.send_cmd(StateCommand::AddUserMessage {
                        content: self.input_buffer.clone(),
                        new_user_msg_id,
                        completion_tx,
                    });
                    self.send_cmd(StateCommand::ScanForChange { scan_tx });
                    self.send_cmd(StateCommand::EmbedMessage {
                        new_msg_id: new_user_msg_id,
                        completion_rx,
                        scan_rx,
                    });
                    self.send_cmd(StateCommand::AddMessage {
                        kind: MessageKind::SysInfo,
                        content: "Embedding User Message".to_string(),
                        target: llm::ChatHistoryTarget::Main,
                        parent_id: new_user_msg_id,
                        child_id: next_llm_msg_id,
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
            Action::OpenContextSearch => todo!(),
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

    fn open_model_browser(&mut self, keyword: String, items: Vec<models::ResponseItem>) {
        let items = Self::build_model_browser_items(items);
        self.model_browser = Some(ModelBrowserState {
            visible: true,
            keyword,
            selected: 0,
            items,
            help_visible: false,
            provider_select_active: false,
            provider_selected: 0,
            vscroll: 0,
            viewport_height: 0,
        });
        self.needs_redraw = true;
    }

    fn open_embedding_browser(&mut self, keyword: String, items: Vec<models::ResponseItem>) {
        let items = Self::build_embedding_browser_items(items);
        self.embedding_browser = Some(EmbeddingBrowserState {
            visible: true,
            keyword,
            selected: 0,
            items,
            help_visible: false,
            vscroll: 0,
            viewport_height: 0,
        });
        self.needs_redraw = true;
    }

    #[instrument(skip(self),
        level = "debug",
        fields(
            search_input,
            retrieved_items_len = retrieved_items.len(),
            self.context_browser
        )
    )]
    #[instrument(
        skip(self, retrieved_items),
        fields(search_input, retrieved_items_len = retrieved_items.len())
    )]
    fn open_context_browser(&mut self, search_input: String, retrieved_items: Vec<ContextPart>) {
        let search_items = Self::build_context_search_items(retrieved_items);
        self.context_browser = Some(ContextSearchState::with_items(search_input, search_items));
        self.needs_redraw = true;
    }

    fn context_browser_needs_tick(&self) -> bool {
        self.context_browser
            .as_ref()
            .map(|cb| cb.pending_dispatch)
            .unwrap_or(false)
    }

    fn tick_context_browser(&mut self) {
        let mut query_to_dispatch: Option<String> = None;
        if let Some(cb) = self.context_browser.as_mut() {
            if !cb.pending_dispatch {
                return;
            }
            if cb.last_edit_at.elapsed() < Duration::from_millis(cb.debounce_ms) {
                return;
            }
            let query = cb.input.as_str().trim().to_string();
            if query == cb.last_sent_query {
                cb.pending_dispatch = false;
                cb.loading_search = false;
                return;
            }
            query_to_dispatch = Some(query);
        }

        if let Some(query) = query_to_dispatch {
            self.dispatch_context_search(&query);
            self.needs_redraw = true;
        }
    }

    fn dispatch_context_search(&mut self, query: &str) {
        let query = query.trim();
        let Some(cb) = self.context_browser.as_mut() else {
            return;
        };
        cb.query_id = cb.query_id.saturating_add(1);
        let query_id = cb.query_id;
        cb.last_sent_query = query.to_string();
        cb.pending_dispatch = false;
        cb.loading_search = true;

        commands::exec::open_context_search(self, query_id, query);
    }

    #[instrument(
        level = "debug",
        fields(retrieved_items_len = retrieved_items.len())
    )]
    fn build_context_search_items(retrieved_items: Vec<ContextPart>) -> Vec<SearchItem> {
        retrieved_items
            .into_iter()
            .map(SearchItem::from)
            .collect_vec()
    }

    fn build_model_browser_items(items: Vec<models::ResponseItem>) -> Vec<ModelBrowserItem> {
        items
            .into_iter()
            .map(|m| {
                let supports_tools = ploke_llm::SupportsTools::supports_tools(&m);
                // Model-level tools: true if any provider supports tools OR model supported_parameters says so
                ModelBrowserItem {
                    id: m.id.clone(),
                    name: Some(m.name),
                    context_length: m.context_length.or(m.top_provider.context_length),
                    // Display pricing in USD per 1M tokens (aligns with provider rows)
                    input_cost: Some(m.pricing.prompt * 1_000_000.0),
                    output_cost: Some(m.pricing.completion * 1_000_000.0),
                    supports_tools,
                    // Provider rows populated later
                    providers: Vec::new(),
                    expanded: false,
                    loading_providers: false,
                    pending_select: false,
                }
            })
            .collect::<Vec<_>>()
    }

    fn build_embedding_browser_items(
        items: Vec<models::ResponseItem>,
    ) -> Vec<EmbeddingBrowserItem> {
        items
            .into_iter()
            .map(|m| {
                let top_provider = m.top_provider.clone();
                let context_length = m.context_length.or(top_provider.context_length);
                EmbeddingBrowserItem {
                    id: m.id.clone(),
                    name: m.name,
                    created: m.created,
                    architecture: m.architecture.clone(),
                    top_provider,
                    pricing: m.pricing,
                    canonical: m.canonical.clone(),
                    context_length,
                    hugging_face_id: m.hugging_face_id.clone(),
                    per_request_limits: m.per_request_limits.clone(),
                    supported_parameters: m.supported_parameters.clone(),
                    description: m.description.clone(),
                    detail: EmbeddingDetail::Collapsed,
                }
            })
            .collect::<Vec<_>>()
    }

    fn close_model_browser(&mut self) {
        self.model_browser = None;
        self.needs_redraw = true;
    }

    fn close_embedding_browser(&mut self) {
        self.embedding_browser = None;
        self.needs_redraw = true;
    }

    fn switch_to_model(&mut self, model_id: &str) {
        // Update runtime active model selection via llm types
        let state = Arc::clone(&self.state);
        let mid = model_id.to_string();
        tokio::task::block_in_place(|| {
            use std::str::FromStr;
            tokio::runtime::Handle::current().block_on(async move {
                match crate::llm::ModelId::from_str(&mid) {
                    Ok(parsed) => {
                        let mut cfg = state.config.write().await;
                        cfg.model_registry
                            .models
                            .entry(parsed.key.clone())
                            .or_default();
                        cfg.active_model = parsed;
                    }
                    Err(e) => tracing::error!("Failed to write model to registry"),
                }
            })
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
        let style = self.command_style;
        let cmd = &self.input_buffer.clone();
        let command = commands::parser::parse(self, cmd, style);
        commands::exec::execute(self, command);
    }

    fn show_command_help(&self) {
        self.send_cmd(StateCommand::AddMessageImmediate {
            msg: commands::HELP_COMMANDS.to_string(),
            kind: MessageKind::SysInfo,
            new_msg_id: Uuid::new_v4(),
        });
    }

    /// Lists all registered endpoint configurations in the chat window.
    ///
    /// Reads the current provider registry from shared state (blocking only the
    /// calling thread) and emits a nicely-formatted list of available models,
    /// including both their short alias and the full model name.
    fn list_models(&self) {
        let cfg = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { self.state.config.read().await })
        });

        let mut lines = vec!["Available models:".to_string()];

        for mk in cfg.model_registry.models.keys() {
            lines.push(format!("{:<4}", mk));
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
