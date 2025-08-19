mod database;
mod models;

// Re-export crate root items so submodules can `use super::*` like the prior monolithic file.
use super::*;

// Internal modules for a modularized AppState
pub mod commands;
mod core;
mod dispatcher;
mod events;
pub mod handlers;
mod helpers;

// Public re-exports to keep external API stable
pub use commands::{ListNavigation, StateCommand, StateError};
pub use core::{
    AppState, ChatState, Config, ConfigState, IndexingState, SystemState, SystemStatus,
};
pub use dispatcher::state_manager;
pub use events::MessageUpdatedEvent;

// Keep tests colocated under app_state after refactor
#[cfg(test)]
mod tests;
