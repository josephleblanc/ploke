mod database;
mod models;

// Re-export crate root items so submodules can `use super::*` like the prior monolithic file.
use super::*;

// Internal modules for a modularized AppState
pub mod commands;
pub mod core;
mod dispatcher;
pub mod events;
pub mod handlers;
mod helpers;

// Public re-exports to keep external API stable
pub use commands::{ListNavigation, StateCommand, StateError};
pub use core::{
    AppState, ChatState, ConfigState, IndexingState, RuntimeConfig, SystemState, SystemStatus,
};
pub use dispatcher::state_manager;
pub use events::MessageUpdatedEvent;

// Test-only exports (use the crate feature so integration tests can call these helpers)
#[cfg(feature = "test_harness")]
pub use database::test_set_crate_focus_from_db;

// Keep tests colocated under app_state after refactor
#[cfg(test)]
mod tests;
