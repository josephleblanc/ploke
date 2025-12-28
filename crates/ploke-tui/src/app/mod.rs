use crate::app::view::components::context_browser::{ContextSearchState, SearchItem};
use crate::chat_history::{ContextTokens, ConversationTotals};
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
pub mod overlay;
pub mod overlay_invariants;
pub mod overlay_manager;
pub mod types;
pub mod utils;
pub mod view;

use super::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::app::input::keymap::{Action, to_action};
use crate::app::overlay::OverlayAction;
use crate::app::overlay_manager::OverlayManager;
use crate::app::types::{Mode, RenderMsg};
use crate::app::utils::truncate_uuid;
use crate::app::message_item::should_render_tool_buttons;
use crate::app::view::components::conversation::ConversationView;
use crate::app::view::components::input_box::{CommandSuggestion, InputView};
use crate::emit_app_event;
use crate::tools::ToolVerbosity;
use crate::ui_theme::UiTheme;
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
use ploke_core::tool_types::ToolName;
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
use view::components::approvals::{ApprovalListItem, ApprovalsState, ProposalKind, filtered_items};
use view::components::config_overlay::ConfigOverlayState;
use view::components::embedding_browser::{
    EmbeddingBrowserItem, EmbeddingBrowserState, EmbeddingDetail,
};
use view::components::model_browser::{ModelBrowserItem, ModelBrowserState};
use crate::app::commands::COMMAND_ENTRIES;

fn compute_input_height(
    desired_input_height: u16,
    frame_height: u16,
    has_indexing: bool,
    show_indicator: bool,
    pending_banner_height: u16,
) -> u16 {
    let min_input_height = 3_u16;
    let max_by_screen = frame_height / 2;
    let mut fixed_height = 1_u16 + 1_u16 + 1_u16; // model info + status + minimum chat
    fixed_height = fixed_height.saturating_add(pending_banner_height);
    if has_indexing {
        fixed_height = fixed_height.saturating_add(3);
    }
    if show_indicator {
        fixed_height = fixed_height.saturating_add(1);
    }
    let max_by_layout = frame_height.saturating_sub(fixed_height);
    let max_input_height = max_by_screen.min(max_by_layout).max(1);
    if max_input_height < min_input_height {
        max_input_height
    } else {
        desired_input_height.clamp(min_input_height, max_input_height)
    }
}

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
    // Overlay manager (config + other overlays)
    overlay_manager: OverlayManager,
    // UI theme (colors)
    theme: UiTheme,
    // Input history browsing (Insert mode)
    input_history: Vec<String>,
    input_history_pos: Option<usize>,
    tool_verbosity: ToolVerbosity,
    confirmation_states: HashMap<Uuid, bool>,
}

impl App {
    /// Construct a new instance of [`App`].
    pub fn new(
        command_style: CommandStyle,
        state: Arc<AppState>,
        cmd_tx: mpsc::Sender<StateCommand>,
        event_bus: &EventBus, // reference non-Arc OK because only created at startup
        active_model_id: String,
        tool_verbosity: ToolVerbosity,
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
            overlay_manager: OverlayManager::default(),
            theme: UiTheme::default(),
            input_history: Vec::new(),
            input_history_pos: None,
            tool_verbosity,
            confirmation_states: HashMap::new(),
        }
    }

    fn apply_tool_verbosity(&mut self, verbosity: ToolVerbosity, announce: bool) {
        self.tool_verbosity = verbosity;
        let state = self.state.clone();
        let cmd_tx = self.cmd_tx.clone();
        tokio::spawn(async move {
            {
                let mut cfg = state.config.write().await;
                cfg.tool_verbosity = verbosity;
            }
            if announce {
                let _ = cmd_tx
                    .send(StateCommand::AddMessageImmediate {
                        msg: format!("Tool verbosity set to {}", verbosity.as_str()),
                        kind: MessageKind::SysInfo,
                        new_msg_id: Uuid::new_v4(),
                    })
                    .await;
            }
        });
    }

    fn cycle_tool_verbosity(&mut self) {
        let next = match self.tool_verbosity {
            ToolVerbosity::Minimal => ToolVerbosity::Normal,
            ToolVerbosity::Normal => ToolVerbosity::Verbose,
            ToolVerbosity::Verbose => ToolVerbosity::Minimal,
        };
        self.apply_tool_verbosity(next, true);
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
        let overlay_tick = tokio::time::sleep(Duration::from_millis(30));
        tokio::pin!(overlay_tick);

        // let mut frame_counter = 0;
        while self.running {
            if self.needs_redraw {
                // Prepare data for this frame by reading from AppState without allocating per-frame.
                let app_state = Arc::clone(&self.state);
                let history_guard = app_state.chat.0.read().await;
                let path_len = history_guard.path_len();
                let current_id = history_guard.current;
                let current_token_totals: Option<ContextTokens> =
                    history_guard.current_context_tokens;

                // Draw the UI using iterators over the cached path.
                terminal.draw(|frame| {
                    self.draw(
                        frame,
                        history_guard.iter_path(),
                        history_guard.iter_path(),
                        path_len,
                        current_id,
                        current_token_totals,
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
                                    if self.input_view.is_input_hovered(
                                        mouse_event.column,
                                        mouse_event.row,
                                    ) {
                                        self.input_view.scroll_prev();
                                    } else {
                                        self.conversation.scroll_lines_up(3);
                                        self.conversation.set_free_scrolling(true);
                                    }
                                    self.pending_char = None;
                                    self.needs_redraw = true;
                                }
                                MouseEventKind::ScrollDown => {
                                    if self.input_view.is_input_hovered(
                                        mouse_event.column,
                                        mouse_event.row,
                                    ) {
                                        self.input_view.scroll_next();
                                    } else {
                                        self.conversation.scroll_lines_down(3);
                                        self.conversation.set_free_scrolling(true);
                                    }
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

            // Trait-based overlay ticks (no-op unless active overlay implements tick).
            _ = &mut overlay_tick => {
                self.overlay_manager.tick(Duration::from_millis(30));
                overlay_tick.as_mut().reset(TokioInstant::now() + Duration::from_millis(30));
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

    fn file_completion(&self, input: &str) -> Option<(String, String)> {
        let at_idx = input.rfind('@')?;
        let after_at = &input[at_idx + 1..];
        if after_at.chars().any(char::is_whitespace) {
            return None;
        }

        let cwd = std::env::current_dir().ok()?;
        if after_at.is_empty() {
            let mut ghost = cwd.display().to_string();
            if !ghost.ends_with(std::path::MAIN_SEPARATOR) {
                ghost.push(std::path::MAIN_SEPARATOR);
            }
            let accept = format!("{}{}", input, ghost);
            return Some((ghost, accept));
        }

        let fragment_path = std::path::Path::new(after_at);
        let (parent, prefix) = if after_at.ends_with(std::path::MAIN_SEPARATOR) {
            (fragment_path, "")
        } else {
            let parent = fragment_path.parent().unwrap_or_else(|| std::path::Path::new(""));
            let prefix = fragment_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            (parent, prefix)
        };

        let search_root = if parent.is_absolute() {
            parent.to_path_buf()
        } else {
            cwd.join(parent)
        };

        let mut matches: Vec<(String, bool)> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&search_root) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if !name.starts_with(prefix) {
                    continue;
                }
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                matches.push((name.to_string(), is_dir));
            }
        }

        if matches.is_empty() {
            return None;
        }

        matches.sort_by(|a, b| a.0.cmp(&b.0));
        let (name, is_dir) = &matches[0];
        let mut ghost = name
            .chars()
            .skip(prefix.chars().count())
            .collect::<String>();
        if *is_dir && !ghost.ends_with(std::path::MAIN_SEPARATOR) {
            ghost.push(std::path::MAIN_SEPARATOR);
        }

        if ghost.is_empty() {
            return None;
        }

        let accept = format!("{}{}", input, ghost);
        Some((ghost, accept))
    }

    fn command_completions(
        &self,
        mode: Mode,
    ) -> (Vec<CommandSuggestion>, Option<String>, Option<String>) {
        if mode != Mode::Command {
            return (Vec::new(), None, None);
        }

        let mut chars = self.input_buffer.chars();
        let Some(prefix) = chars.next() else {
            return (Vec::new(), None, None);
        };
        if prefix != '/' && prefix != ':' {
            return (Vec::new(), None, None);
        }

        if let Some((ghost, accept)) = self.file_completion(&self.input_buffer) {
            return (Vec::new(), Some(ghost), Some(accept));
        }

        let typed = chars.as_str();
        if typed.is_empty() {
            return (Vec::new(), None, None);
        }
        let typed_lower = typed.to_lowercase();

        let matches: Vec<&crate::app::commands::CommandEntry> = COMMAND_ENTRIES
            .iter()
            .filter(|entry| entry.command.starts_with(&typed_lower))
            .take(10)
            .collect();

        if matches.is_empty() {
            return (Vec::new(), None, None);
        }

        let suggestions = matches
            .iter()
            .map(|entry| CommandSuggestion {
                command: format!("{prefix}{}", entry.completion),
                description: entry.description.to_string(),
            })
            .collect::<Vec<_>>();

        let first = matches[0].completion;
        let typed_len = typed.chars().count();
        let ghost_text = first
            .chars()
            .skip(typed_len)
            .collect::<String>();

        let mut accept_text = format!("{prefix}{}", matches[0].command);
        if matches[0].completion != matches[0].command {
            accept_text.push(' ');
        }

        (suggestions, Some(ghost_text), Some(accept_text))
    }

    /// Count pending edit proposals for UI banner display.
    fn pending_edit_count(&self) -> usize {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let reg = self.state.proposals.read().await;
                reg.values()
                    .filter(|p| {
                        matches!(
                            p.status,
                            crate::app_state::core::EditProposalStatus::Pending
                        )
                    })
                    .count()
            })
        })
    }

    /// Renders the user interface.
    fn draw<'a, I1, I2, T: RenderMsg + 'a>(
        &mut self,
        frame: &mut Frame,
        path_for_measure: I1,
        path_for_render: I2,
        path_len: usize,
        current_id: Uuid,
        current_token_totals: Option<ContextTokens>,
    ) where
        I1: IntoIterator<Item = &'a T> + Clone,
        I2: IntoIterator<Item = &'a T>,
    {
        let input_mode = if self.overlay_manager.is_active() {
            Mode::Normal
        } else {
            self.mode
        };
        let (command_suggestions, ghost_text, accept_text) =
            self.command_completions(input_mode);
        let pending_edits = self.pending_edit_count();
        let pending_banner_height = if pending_edits > 0 { 1 } else { 0 };
        // Always show the currently selected model in the top-right
        let show_indicator = true;
        let frame_area = frame.area();
        let desired_input_height =
            self.input_view
                .desired_height(&self.input_buffer, frame_area.width)
                .saturating_add(command_suggestions.len() as u16);
        let input_height = compute_input_height(
            desired_input_height,
            frame_area.height,
            self.indexing_state.is_some(),
            show_indicator,
            pending_banner_height,
        );

        // ---------- Define Layout ----------
        let mut proto_layout = vec![Constraint::Length(1), Constraint::Min(1)];
        let pending_banner_idx_opt = if pending_banner_height > 0 {
            let idx = proto_layout.len();
            proto_layout.push(Constraint::Length(pending_banner_height));
            Some(idx)
        } else {
            None
        };
        let input_idx = {
            let idx = proto_layout.len();
            proto_layout.push(Constraint::Length(input_height));
            idx
        };
        let status_idx = {
            let idx = proto_layout.len();
            proto_layout.push(Constraint::Length(1));
            idx
        };
        let indexing_idx_opt = if self.indexing_state.is_some() {
            let idx = proto_layout.len();
            proto_layout.push(Constraint::Length(3));
            Some(idx)
        } else {
            None
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
        let pending_banner_area_opt = pending_banner_idx_opt.map(|idx| main_layout[idx]);
        let input_area = main_layout[input_idx];
        let status_area = main_layout[status_idx];

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

        let status_line_area = layout_statusline(4, status_area);

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
            &self.confirmation_states,
        );

        // Right-side context preview (placeholder until wired to Rag events)
        if let Some(preview_area) = preview_area_opt {
            let preview = Paragraph::new("Context Preview\nWaiting for results…")
                .block(Block::bordered().title(" Context Preview "));
            frame.render_widget(preview, preview_area);
        }

        if let Some(pending_banner_area) = pending_banner_area_opt {
            let banner = Paragraph::new(format!(
                "Pending edit proposals: {}  |  Shift+Y approve all, Shift+N reject all",
                pending_edits
            ))
            .style(
                Style::new()
                    .fg(self.theme.input_command_fg)
                    .bg(self.theme.input_suggestion_bg),
            )
            .alignment(ratatui::layout::Alignment::Left);
            frame.render_widget(banner, pending_banner_area);
        }

        // Render input box via InputView
        self.input_view.render(
            frame,
            input_area,
            &self.input_buffer,
            input_mode,
            &self.theme,
            ghost_text.as_deref(),
            &command_suggestions,
        );
        // Add progress bar at bottom if indexing
        if let (Some(state), Some(indexing_idx)) = (&self.indexing_state, indexing_idx_opt) {
            let progress_block = Block::default().borders(Borders::TOP).title(" Indexing ");

            let gauge = Gauge::default()
                .block(progress_block)
                .ratio(state.calc_progress())
                .gauge_style(Style::new().light_blue());

            frame.render_widget(gauge, main_layout[indexing_idx]); // Bottom area
        }

        // Render Mode to text
        let status_bar = Block::default()
            .title(self.mode.to_string())
            .borders(Borders::NONE)
            .padding(Padding::vertical(1));
        let node_status = Paragraph::new(format!("Node: {}", truncate_uuid(current_id)))
            .block(Block::default().borders(Borders::NONE))
            .style(Style::new().fg(Color::Blue));
        let context_tracker = {
            let fmt_arg = if let Some(current_tokens) = current_token_totals {
                format!("{}", current_tokens.count)
            } else {
                "unknown".to_string()
            };
            let formatted_tokens = format!("ctx tokens: {}", fmt_arg);

            Paragraph::new(formatted_tokens)
                .block(Block::default().borders(Borders::NONE))
                .style(Style::new().fg(Color::Blue))
        };

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
        frame.render_widget(context_tracker, status_line_area[3]);

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

        // Render overlay manager if active
        if self.overlay_manager.is_active() {
            self.overlay_manager.render(frame, &self.state);
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
    }

    /// Handles the key events and updates application state via high-level Actions.
    ///
    /// Phase 1 refactor: convert KeyEvent -> Action in input::keymap, then handle here.
    fn on_key_event(&mut self, key: KeyEvent) {
        // Intercept overlay manager keys
        if self.overlay_manager.is_active() {
            let actions = self.overlay_manager.handle_input(key);
            self.handle_overlay_actions(actions);
            self.needs_redraw = true;
            return;
        }
        // Overlay manager handles overlay input above.

        // Global action mapping (including OpenApprovals)
        if let Some(action) = to_action(self.mode, key, self.command_style)
            && Action::OpenApprovals == action
        {
            if self.overlay_manager.is_approvals_open() {
                self.overlay_manager.close_active();
            } else {
                self.overlay_manager
                    .open_approvals(ApprovalsState::default());
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
                if self.overlay_manager.is_approvals_open() {
                    self.overlay_manager.close_active();
                } else {
                    self.overlay_manager
                        .open_approvals(ApprovalsState::default());
                }
            }
            Action::OpenConfigOverlay => {
                if self.overlay_manager.is_config_open() {
                    self.overlay_manager.close_active();
                } else {
                    self.open_config_overlay();
                }
            }
            Action::ApproveAllPendingEdits => {
                self.send_cmd(StateCommand::ApprovePendingEdits);
            }
            Action::DenyAllPendingEdits => {
                self.send_cmd(StateCommand::DenyPendingEdits);
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
            Action::AcceptCompletion => {
                if self.mode == Mode::Command {
                    let (_, _, accept_text) = self.command_completions(self.mode);
                    if let Some(accept) = accept_text {
                        self.input_buffer = accept;
                    }
                }
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
                let mut handled = false;
                if let Some(selected) = self.list.selected() {
                    if let Some(msg_id) = self.conversation.interactive_tools.get(&selected) {
                        self.confirmation_states.insert(*msg_id, true);
                        handled = true;
                        self.needs_redraw = true;
                    }
                }

                if !handled {
                    self.conversation.set_free_scrolling(false);
                    self.pending_char = None;
                    self.send_cmd(StateCommand::NavigateBranch {
                        direction: Previous,
                    });
                }
            }
            Action::BranchNext => {
                let mut handled = false;
                if let Some(selected) = self.list.selected() {
                    if let Some(msg_id) = self.conversation.interactive_tools.get(&selected) {
                        self.confirmation_states.insert(*msg_id, false);
                        handled = true;
                        self.needs_redraw = true;
                    }
                }

                if !handled {
                    self.conversation.set_free_scrolling(false);
                    self.pending_char = None;
                    self.send_cmd(StateCommand::NavigateBranch { direction: Next });
                }
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
            Action::ToggleToolVerbosity => {
                self.pending_char = None;
                self.cycle_tool_verbosity();
            }
            Action::InputScrollPrev => {
                self.input_view.scroll_prev();
            }
            Action::InputScrollNext => {
                self.input_view.scroll_next();
            }
            Action::OpenContextSearch => todo!(),
            Action::TriggerSelection => {
                if let Some(selected) = self.list.selected() {
                    let should_trigger = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            let guard = self.state.chat.0.read().await;
                            let path = guard.get_full_path();
                            if let Some(msg) = path.get(selected) {
                                if let Some(payload) = msg.tool_payload() {
                                    if should_render_tool_buttons(payload) {
                                        if let Some(req_id) = payload.request_id {
                                            return Some(req_id);
                                        }
                                    }
                                }
                            }
                            None
                        })
                    });

                    if let Some(request_id) = should_trigger {
                        let is_yes = self
                            .confirmation_states
                            .get(&self.conversation.interactive_tools.get(&selected).copied().unwrap_or_default())
                            .copied()
                            .unwrap_or(true);

                        if is_yes {
                            self.send_cmd(StateCommand::ApproveEdits { request_id });
                        } else {
                            self.send_cmd(StateCommand::DenyEdits { request_id });
                        }
                    }
                }
            }
        }
    }

    fn handle_overlay_actions(&mut self, actions: Vec<OverlayAction>) {
        for action in actions {
            match action {
                OverlayAction::CloseOverlay(kind) => {
                    self.overlay_manager.close_kind(kind);
                }
                OverlayAction::RequestModelEndpoints { model_id } => {
                    self.request_model_endpoints(model_id);
                }
                OverlayAction::SelectModel { model_id, provider } => {
                    self.apply_model_provider_selection(model_id.to_string(), provider);
                    self.overlay_manager
                        .close_kind(overlay::OverlayKind::ModelBrowser);
                }
                OverlayAction::SelectEmbeddingModel { model_id, provider } => {
                    self.apply_embedding_model_selection(model_id, provider);
                    self.overlay_manager
                        .close_kind(overlay::OverlayKind::EmbeddingBrowser);
                }
                OverlayAction::ApproveSelectedProposal => {
                    self.handle_selected_approval(true);
                }
                OverlayAction::DenySelectedProposal => {
                    self.handle_selected_approval(false);
                }
                OverlayAction::OpenSelectedProposalInEditor => {
                    self.open_selected_proposal_in_editor();
                }
            }
        }
    }

    fn handle_selected_approval(&mut self, approve: bool) {
        let Some(st) = self.overlay_manager.approvals_state() else {
            return;
        };
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

    fn open_selected_proposal_in_editor(&mut self) {
        let Some(st) = self.overlay_manager.approvals_state() else {
            return;
        };
        let sel_index = st.selected;
        let filter = st.filter;
        let state = Arc::clone(&self.state);
        let cmd_tx = self.cmd_tx.clone();
        tokio::spawn(async move {
            // Build unified ordering to match overlay
            let items = filtered_items(&state, filter);
            if let Some(ApprovalListItem { kind, id, .. }) = items.get(sel_index).cloned() {
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
                        let _ = cmd_tx.try_send(StateCommand::AddMessageImmediate {
                            msg: "No editor configured. Set PLOKE_EDITOR or config ploke_editor."
                                .into(),
                            kind: MessageKind::SysInfo,
                            new_msg_id: uuid::Uuid::new_v4(),
                        });
                    }
                }
            }
        });
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
        self.overlay_manager.open_model_browser(ModelBrowserState {
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

    fn open_config_overlay(&mut self) {
        let cfg = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async { self.state.config.read().await })
        });
        self.overlay_manager
            .open_config(ConfigOverlayState::from_runtime_config(&cfg));
        self.needs_redraw = true;
    }

    fn open_embedding_browser(&mut self, keyword: String, items: Vec<models::ResponseItem>) {
        let items = Self::build_embedding_browser_items(items);
        self.overlay_manager
            .open_embedding_browser(EmbeddingBrowserState {
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
            retrieved_items_len = retrieved_items.len()
        )
    )]
    #[instrument(
        skip(self, retrieved_items),
        fields(search_input, retrieved_items_len = retrieved_items.len())
    )]
    fn open_context_browser(&mut self, search_input: String, retrieved_items: Vec<ContextPart>) {
        let search_items = Self::build_context_search_items(retrieved_items);
        self.overlay_manager
            .open_context_browser(ContextSearchState::with_items(search_input, search_items));
        self.needs_redraw = true;
    }

    fn context_browser_needs_tick(&self) -> bool {
        self.overlay_manager
            .context_state()
            .map(|cb| cb.pending_dispatch)
            .unwrap_or(false)
    }

    fn tick_context_browser(&mut self) {
        let mut query_to_dispatch: Option<String> = None;
        if let Some(cb) = self.overlay_manager.context_state_mut() {
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
        let Some(cb) = self.overlay_manager.context_state_mut() else {
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

    fn request_model_endpoints(&self, model_id: ModelId) {
        tokio::spawn(async move {
            let router = RouterVariants::OpenRouter(OpenRouter);
            emit_app_event(
                LlmEvent::Endpoint(endpoint::Event::Request {
                    model_key: model_id.key,
                    router,
                    variant: model_id.variant,
                })
                .into(),
            )
            .await;
        });
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
        self.overlay_manager
            .open_approvals(ApprovalsState::default());
    }

    /// Close the approvals overlay (intended for tests and scripted UI flows)
    pub fn approvals_close(&mut self) {
        self.overlay_manager
            .close_kind(overlay::OverlayKind::Approvals);
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

#[cfg(test)]
mod tests {
    use super::compute_input_height;
    use crate::test_utils::mock::create_mock_app;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn input_height_clamps_to_screen_and_layout() {
        let height = compute_input_height(50, 20, false, true, 0);
        assert_eq!(height, 10);
    }

    #[test]
    fn input_height_uses_min_when_possible() {
        let height = compute_input_height(3, 20, false, true, 0);
        assert_eq!(height, 3);
    }

    #[test]
    fn input_height_falls_back_when_screen_is_tiny() {
        let height = compute_input_height(10, 5, false, true, 0);
        assert_eq!(height, 1);
    }

    struct CwdGuard {
        prev: PathBuf,
    }

    impl CwdGuard {
        fn set_to(path: &std::path::Path) -> Self {
            let prev = std::env::current_dir().expect("current dir");
            std::env::set_current_dir(path).expect("set current dir");
            Self { prev }
        }
    }

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.prev);
        }
    }

    #[test]
    fn file_completion_uses_cwd_for_bare_at() {
        let temp = tempdir().expect("temp dir");
        let _guard = CwdGuard::set_to(temp.path());
        let app = create_mock_app();

        let input = "/model load @";
        let (ghost, accept) = app.file_completion(input).expect("completion");

        let mut expected_ghost = temp.path().display().to_string();
        if !expected_ghost.ends_with(std::path::MAIN_SEPARATOR) {
            expected_ghost.push(std::path::MAIN_SEPARATOR);
        }
        assert_eq!(ghost, expected_ghost);
        assert_eq!(accept, format!("{input}{expected_ghost}"));
    }

    #[test]
    fn file_completion_resolves_temp_entries() {
        let temp = tempdir().expect("temp dir");
        std::fs::create_dir(temp.path().join("src")).expect("create src");
        std::fs::write(temp.path().join("Cargo.toml"), "cargo").expect("write file");
        std::fs::write(temp.path().join("src").join("main.rs"), "fn main() {}")
            .expect("write file");
        let _guard = CwdGuard::set_to(temp.path());
        let app = create_mock_app();

        let input_dir = "/open @s";
        let (ghost_dir, accept_dir) = app.file_completion(input_dir).expect("dir completion");
        assert_eq!(ghost_dir, format!("rc{}", std::path::MAIN_SEPARATOR));
        assert_eq!(accept_dir, format!("{input_dir}rc{}", std::path::MAIN_SEPARATOR));

        let input_file = "/open @src/m";
        let (ghost_file, accept_file) =
            app.file_completion(input_file).expect("file completion");
        assert_eq!(ghost_file, "ain.rs");
        assert_eq!(accept_file, "/open @src/main.rs");
    }
}
