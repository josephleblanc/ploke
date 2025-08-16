mod database;
mod models;

// Re-export crate root items so submodules can `use super::*` like the prior monolithic file.
pub use crate::*;

// Internal modules for a modularized AppState
mod core;
pub mod commands;
mod events;
mod dispatcher;
mod helpers;
pub mod handlers;

// Public re-exports to keep external API stable
pub use core::{AppState, ChatState, Config, ConfigState, IndexingState, SystemState, SystemStatus};
pub use commands::{ListNavigation, StateCommand, StateError};
pub use dispatcher::state_manager;
pub use events::MessageUpdatedEvent;

// Keep tests colocated under app_state after refactor
#[cfg(test)]
mod tests;
