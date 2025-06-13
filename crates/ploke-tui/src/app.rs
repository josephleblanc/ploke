// src/app.rs
use crate::Config;
use std::collections::VecDeque;

/// Represents the current mode of the application.
#[derive(Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal, // For navigation, general commands
    Input, // For typing user queries
}

#[derive(Debug, PartialEq, Eq)]
pub enum ModalType {
    QuitConfirm,
}

/// Events that can be sent to the App for state updates.
#[derive(Debug)]
pub enum AppEvent {
    /// A key press event from the terminal.
    Key(crossterm::event::KeyEvent),
    /// A terminal resize event.
    Resize(u16, u16),
    /// A message received from the backend (e.g., an LLM response).
    BackendResponse {
        model: String,
        content: String,
    },
    /// A request to send to the backend.
    SendQuery(String),
    /// Request to quit the application.
    Quit,
}

/// Represents the application's state.
#[derive(Debug)]
pub struct App {
    pub mode: Mode,
    pub current_input: String,
    pub messages: VecDeque<String>, // Using VecDeque for efficient pop_front/push_back
    pub should_quit: bool,
    pub backend_tx: flume::Sender<BackendRequest>, // Channel to send requests to the backend
    pub active_modals: Vec<ModalType>,
    pub config: Config,
}

impl App {
    pub fn new(backend_tx: flume::Sender<BackendRequest>, config: Config) -> Self {
        Self {
            mode: Mode::default(),
            current_input: String::new(),
            messages: VecDeque::with_capacity(config.max_history),
            config,
            should_quit: false,
            backend_tx,
            active_modals: Vec::new(),
            history_scroll_offset: 0,
        }
    }

    /// Updates the application state based on an `AppEvent`.
    pub fn update(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key_event) => self.handle_key_event(key_event),
            AppEvent::Resize(_, _) => {
                // In a real app, you might re-calculate layouts here
                // For now, ratatui handles basic resizing automatically
            }
            AppEvent::BackendResponse { model, content } => {
                self.messages.push_back(format!("{}: {}", model, content));
                if self.messages.len() > self.messages.capacity() {
                    self.messages.pop_front(); // Keep history within capacity
                }
                self.history_scroll_offset = usize::MAX;
            }
            AppEvent::SendQuery(query) => {
                self.messages.push_back(format!("You: {}", query));
                if self.messages.len() > self.messages.capacity() {
                    self.messages.pop_front();
                }
                self.history_scroll_offset = usize::MAX;
                // Send the query to the backend
                let _ = self.backend_tx.send(BackendRequest::Query(query));
            }
            AppEvent::Quit => self.should_quit = true,
        }
    }

    /// Handles individual key events based on the current mode.
    fn handle_key_event(&mut self, key_event: crossterm::event::KeyEvent) {
        use crossterm::event::{KeyCode, KeyModifiers};

        // Global key handlers (work in any mode)
<<<<<<< HEAD
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && key_event.code == KeyCode::Char('c')
=======
        // NOTE: This must be checked before modal specific handling, otherwise if a modal
        // is open, it will intercept Ctrl+C.
        if key_event.modifiers.contains(KeyModifiers::CONTROL) 
            && key_event.code == KeyCode::Char('c') 
>>>>>>> cec5e204d3998bf963c32deebb6ffdbb0edac022
        {
            self.should_quit = true;
            return;
        }

<<<<<<< HEAD
        if let Some(active_modal) = self.active_modals.last() {
            match (active_modal, key_event.code) {
                (ModalType::QuitConfirm, KeyCode::Char('y')) => self.should_quit = true,
                (ModalType::QuitConfirm, KeyCode::Char('n') | KeyCode::Esc) => {
                    self.active_modals.pop();
                }
                _ => {} // Ignore other keys when modal is active
            }
            return; // Modal handling consumes the event
        }
=======
        // Modal handling (applies to topmost modal if any)
        if let Some(top_modal) = self.active_modals.last() {
            match (top_modal, key_event.code) {
                (ModalType::QuitConfirm, KeyCode::Char('y')) => {
                    self.should_quit = true;
                    return; // Consume event
                }
                (ModalType::QuitConfirm, KeyCode::Char('n') | KeyCode::Esc) => {
                    self.active_modals.pop();
                    return; // Consume event
                }
                // If the key is not handled by the active modal,
                // let it fall through to mode-specific handling.
                _ => {}
            }
        }

        // If no active modals, or if the event was not consumed by modal logic,
        // proceed with mode-specific key handling.
>>>>>>> cec5e204d3998bf963c32deebb6ffdbb0edac022
        match self.mode {
            Mode::Normal => match key_event.code {
                KeyCode::Char('q') => self.active_modals.push(ModalType::QuitConfirm),
                KeyCode::Char('i') => self.mode = Mode::Input,
                KeyCode::Char('k') | KeyCode::Up => {
                    self.history_scroll_offset = self.history_scroll_offset.saturating_sub(1);
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    if !self.messages.is_empty() {
                        self.history_scroll_offset = self.history_scroll_offset.saturating_add(1);
                        // Ensure offset doesn't go beyond the last message index
                        if self.history_scroll_offset >= self.messages.len() {
                            self.history_scroll_offset = self.messages.len().saturating_sub(1);
                        }
                    }
                }
                // more here..
                _ => {}
            },
<<<<<<< HEAD
            // Modal handling (applies to topmost modal)
=======
            // The original `_ if !self.active_modals.is_empty()` branch is removed
            // as modal handling is now done above.
>>>>>>> cec5e204d3998bf963c32deebb6ffdbb0edac022
            Mode::Input => match key_event.code {
                // How can we support multiple key presses here? It might be nice to have a
                // "Shift+Enter" configurable option for multi-line input.
                KeyCode::Enter => {
                    if !self.current_input.trim().is_empty() {
                        let query = self.current_input.drain(..).collect();
                        self.update(AppEvent::SendQuery(query));
                    }
                }
                KeyCode::Backspace => {
                    self.current_input.pop();
                }
                KeyCode::Esc => {
                    self.mode = Mode::Normal; // Exit input mode
                    // self.current_input.clear(); // Clear input on escape
                }
                KeyCode::Char(c) => {
                    if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                        // Handle Ctrl+key combinations if needed
                        match c {
                            'c' => self.should_quit = true, // Ctrl+C to quit
                            _ => {}
                        }
                    } else {
                        self.current_input.push(c);
                    }
                }
                _ => {}
            },
        }
    }
}

/// Requests that can be sent to the backend.
#[derive(Debug)]
pub enum BackendRequest {
    Query(String),
    // Add more request types as your backend evolves
}

/// Responses that can be received from the backend.
#[derive(Debug)]
pub enum BackendResponse {
    QueryResult(String),
    // Add more response types
}
