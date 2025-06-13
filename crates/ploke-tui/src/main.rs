// src/main.rs
mod app;
mod ui;
mod events;
mod backend;

use app::{App, AppEvent, BackendRequest, BackendResponse};
use color_eyre::eyre::{self, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::task;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Setup error handling with color-eyre
    // This provides enhanced error messages and backtraces.

    // 2. Initialize the terminal for TUI
    // Enter raw mode to capture key presses directly.
    enable_raw_mode()?;
    // Enter alternate screen to not mess up the user's terminal history.
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 3. Setup Flume Channels for inter-task communication
    // app_event_tx/rx: For sending UI events (key presses, resizes, backend responses) to the App.
    let (app_event_tx, app_event_rx) = flume::unbounded::<AppEvent>();
    // backend_request_tx/rx: For sending requests from the App to the simulated backend.
    let (backend_request_tx, backend_request_rx) = flume::unbounded::<BackendRequest>();

    // 4. Load configuration
    let config = config::Config::builder().add_source(config::File::with_name(".ploke.settings"));
    
    // Create the application state with config
    let mut app = App::new(backend_request_tx.clone(), config); // Clone sender for App to use

    // 5. Spawn background tasks
    // Event listener task: polls for terminal events and sends them to the App.
    let event_tx_clone = app_event_tx.clone();
    let event_handle = task::spawn(async move {
        if let Err(e) = events::start_event_listener(event_tx_clone).await {
            eprintln!("Event listener error: {:?}", e);
        }
    });

    // Backend listener task: receives requests from App and sends responses back to App.
    let backend_handle = task::spawn(async move {
        if let Err(e) = backend::start_backend_listener(backend_request_rx, app_event_tx).await {
            eprintln!("Backend listener error: {:?}", e);
        }
    });

    // 6. Main application loop
    loop {
        // Draw the UI
        terminal.draw(|f| ui::render(f, &app))?;

        // Process events
        // Try to receive an event without blocking. If no event, continue to next frame.
        if let Ok(event) = app_event_rx.try_recv() {
            app.update(event);
        }

        // Check if the app should quit
        if app.should_quit {
            break;
        }

        // Small delay to prevent busy-looping, adjust as needed
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // 7. Cleanup and restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Wait for background tasks to finish gracefully (optional, but good practice)
    event_handle.await?;
    backend_handle.await?;

    Ok(())
}
