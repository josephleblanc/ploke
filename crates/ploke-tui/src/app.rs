// src/app.rs
use std::collections::VecDeque;

/// Represents the current mode of the application.
#[derive(Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal, // For navigation, general commands
    Input,  // For typing user queries
    // Add more modes as needed, e.g., `ViewingGraph`, `Settings`
}

/// Events that can be sent to the App for state updates.
#[derive(Debug)]
pub enum AppEvent {
    /// A key press event from the terminal.
    Key(crossterm::event::KeyEvent),
    /// A terminal resize event.
    Resize(u16, u16),
    /// A message received from the backend (e.g., an LLM response).
    BackendResponse(String),
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
}

impl App {
    pub fn new(backend_tx: flume::Sender<BackendRequest>) -> Self {
        Self {
            mode: Mode::default(),
            current_input: String::new(),
            // TODO: history size arbitrarily limited here. Need to implement a way to limit
            // conversation history based on user preferences in a config, with sane defaults based
            // on model.
            // Follow `aider`s lead here and don't enforce it (at not least without user preference).
            // The user should always be in control.
            messages: VecDeque::with_capacity(100),
            should_quit: false,
            backend_tx,
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
            AppEvent::BackendResponse(response) => {
                // TODO: Change this to have model name, or otherwise something better than "LLM"
                self.messages.push_back(format!("LLM: {}", response));
                if self.messages.len() > self.messages.capacity() {
                    self.messages.pop_front(); // Keep history within capacity
                }
            }
            AppEvent::SendQuery(query) => {
                self.messages.push_back(format!("You: {}", query));
                if self.messages.len() > self.messages.capacity() {
                    self.messages.pop_front();
                }
                // Send the query to the backend
                let _ = self.backend_tx.send(BackendRequest::Query(query));
            }
            AppEvent::Quit => self.should_quit = true,
        }
    }

    /// Handles individual key events based on the current mode.
    fn handle_key_event(&mut self, key_event: crossterm::event::KeyEvent) {
        use crossterm::event::{KeyCode, KeyModifiers};

        match self.mode {
            Mode::Normal => match key_event.code {
                // TODO: Change method of exiting here. Just pressing "q", with no confirmation,
                // makes it too easy to accidentally exit the application.
                KeyCode::Char('q') => self.should_quit = true,
                KeyCode::Char('i') => self.mode = Mode::Input, // Enter input mode

                // more here..
                _ => {}
            },
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
                    self.current_input.clear(); // Clear input on escape
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
